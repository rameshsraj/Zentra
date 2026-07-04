//! LogShipper — tails confirmed log directories discovered on this host and
//! ships log lines to the Zentra server via the OTLP log delivery channel.
//!
//! Flow:
//!   1. On startup (and every 5 min), fetch confirmed log dirs from GET /api/agents/log-dirs
//!   2. For each .log / syslog file found in those directories, tail from the last
//!      known byte offset (persisted in a small state file under buffer_dir)
//!   3. Parse each line into a LogEntry (severity auto-detected) and send as LogBatch

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

use zentra_core::models::log_entry::{LogBatch, LogEntry, LogSeverity};

const REFRESH_INTERVAL_SECS: u64 = 300; // re-fetch log dirs every 5 min
const TAIL_INTERVAL_SECS: u64 = 10;     // poll files every 10s
const MAX_LINES_PER_CYCLE: usize = 1000; // backpressure guard

/// Persisted per-file tail state (byte offset).
#[derive(Debug, Serialize, Deserialize, Default)]
struct TailState {
    offsets: HashMap<String, u64>,
}

impl TailState {
    fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self, path: &Path) {
        if let Ok(s) = serde_json::to_string(self) {
            let _ = std::fs::write(path, s);
        }
    }
}

/// A log directory entry returned by the API.
#[derive(Debug, Deserialize)]
struct LogDirEntry {
    id: Uuid,
    path: String,
}

#[derive(Debug, Deserialize)]
struct LogDirsResponse {
    log_dirs: Vec<LogDirEntry>,
    agent_id: Uuid,
}

pub struct LogShipper {
    api_endpoint: String,
    api_key: String,
    buffer_dir: String,
    log_tx: mpsc::Sender<LogBatch>,
    cancel: CancellationToken,
    client: Client,
}

impl LogShipper {
    pub fn new(
        api_endpoint: String,
        api_key: String,
        buffer_dir: String,
        log_tx: mpsc::Sender<LogBatch>,
        cancel: CancellationToken,
    ) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("zentra-agent/1.0")
            .build()
            .expect("Failed to build HTTP client for LogShipper");

        Self {
            api_endpoint: api_endpoint.trim_end_matches('/').to_string(),
            api_key,
            buffer_dir,
            log_tx,
            cancel,
            client,
        }
    }

    pub async fn run(self) {
        info!("LogShipper started");

        let state_path = PathBuf::from(&self.buffer_dir).join("log_tail_state.json");
        let mut tail_state = TailState::load(&state_path);
        let mut log_dirs: Vec<LogDirEntry> = Vec::new();
        let mut last_refresh = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(REFRESH_INTERVAL_SECS + 1))
            .unwrap_or(std::time::Instant::now());

        let mut agent_id: Option<Uuid> = None;

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("LogShipper: shutdown signal received");
                    tail_state.save(&state_path);
                    return;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(TAIL_INTERVAL_SECS)) => {}
            }

            // Refresh log dir list periodically
            if last_refresh.elapsed().as_secs() >= REFRESH_INTERVAL_SECS {
                match self.fetch_log_dirs().await {
                    Ok(resp) => {
                        agent_id = Some(resp.agent_id);
                        log_dirs = resp.log_dirs;
                        last_refresh = std::time::Instant::now();
                        info!(count = log_dirs.len(), "LogShipper: log dirs refreshed");
                    }
                    Err(e) => {
                        warn!("LogShipper: failed to fetch log dirs: {}", e);
                        // Keep using existing dirs if any
                    }
                }
            }

            if log_dirs.is_empty() {
                continue;
            }

            let aid = match agent_id {
                Some(id) => id,
                None => continue,
            };

            // Collect log files from all confirmed directories
            let log_files = collect_log_files(&log_dirs);

            let mut all_entries: Vec<LogEntry> = Vec::new();

            for file_path in &log_files {
                let path_str = file_path.to_string_lossy().to_string();
                let offset = tail_state.offsets.get(&path_str).copied().unwrap_or(0);

                match tail_file(file_path, offset, aid, &path_str) {
                    Ok((entries, new_offset)) => {
                        if !entries.is_empty() {
                            debug!(
                                file = %path_str,
                                count = entries.len(),
                                "LogShipper: tailed new lines"
                            );
                            tail_state.offsets.insert(path_str, new_offset);
                            all_entries.extend(entries);
                            if all_entries.len() >= MAX_LINES_PER_CYCLE {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        warn!(file = %path_str, err = %e, "LogShipper: tail error");
                        // Reset offset if file was rotated (size < offset)
                        tail_state.offsets.remove(&path_str);
                    }
                }
            }

            tail_state.save(&state_path);

            if !all_entries.is_empty() {
                let batch = LogBatch::new("log-shipper", all_entries);
                if self.log_tx.send(batch).await.is_err() {
                    warn!("LogShipper: log channel closed, shutting down");
                    return;
                }
            }
        }
    }

    async fn fetch_log_dirs(&self) -> Result<LogDirsResponse, String> {
        let url = format!("{}/api/agents/log-dirs", self.api_endpoint);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }

        resp.json::<LogDirsResponse>()
            .await
            .map_err(|e| e.to_string())
    }
}

/// Returns all readable .log / syslog files within the given directories (non-recursive).
fn collect_log_files(dirs: &[LogDirEntry]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let log_extensions = ["log", "out", "err"];
    let syslog_names = ["syslog", "messages", "kern.log", "auth.log", "secure",
                        "cron", "maillog", "dmesg", "boot.log"];

    for entry in dirs {
        let dir = Path::new(&entry.path);
        if !dir.is_dir() {
            continue;
        }
        let read_dir = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for de in read_dir.flatten() {
            let path = de.path();
            if !path.is_file() {
                continue;
            }
            let fname = path.file_name().unwrap_or_default().to_string_lossy();
            // Accept known syslog names or files ending in log extensions
            let is_log = syslog_names.iter().any(|&n| fname == n)
                || path.extension().map(|e| log_extensions.contains(&e.to_str().unwrap_or(""))).unwrap_or(false);
            if is_log {
                files.push(path);
            }
        }
    }
    files
}

/// Tail a file from `offset`, return new log entries and the new offset.
fn tail_file(
    path: &Path,
    offset: u64,
    agent_id: Uuid,
    source: &str,
) -> Result<(Vec<LogEntry>, u64), std::io::Error> {
    let file = std::fs::File::open(path)?;
    let file_size = file.metadata()?.len();

    // File was rotated — reset
    if offset > file_size {
        return Ok((Vec::new(), 0));
    }

    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(offset))?;

    let mut entries = Vec::new();
    let mut bytes_read: u64 = 0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        bytes_read += line.len() as u64 + 1; // +1 for newline
        if line.is_empty() {
            continue;
        }
        let severity = detect_severity(&line);
        let entry = LogEntry::new(
            uuid::Uuid::nil(), // org_id filled server-side from API key
            agent_id,          // target_id = agent_id until linked to real target
            source,
            severity,
            &line,
        )
        .with_raw(line.clone())
        .with_component(
            path.file_name().unwrap_or_default().to_string_lossy(),
            "log_file",
        );
        entries.push(entry);

        if entries.len() >= MAX_LINES_PER_CYCLE {
            break;
        }
    }

    let new_offset = offset + bytes_read;
    Ok((entries, new_offset))
}

/// Detect log severity from common patterns in a log line.
fn detect_severity(line: &str) -> LogSeverity {
    let lower = line.to_lowercase();
    if lower.contains(" error") || lower.contains("[error]") || lower.contains("err:") {
        LogSeverity::Error
    } else if lower.contains(" warn") || lower.contains("[warn]") || lower.contains("warning") {
        LogSeverity::Warning
    } else if lower.contains(" debug") || lower.contains("[debug]") {
        LogSeverity::Debug
    } else if lower.contains(" crit") || lower.contains("[crit]") || lower.contains("fatal") {
        LogSeverity::Error
    } else {
        LogSeverity::Info
    }
}
