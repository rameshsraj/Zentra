//! Newline-delimited JSON offline buffer for remote mode.
//!
//! Batches that fail delivery are appended to a buffer file.
//! On flush, entries are read oldest-first and acknowledged by rewriting
//! the file without the acknowledged prefix.
//!
//! Capped at 10,000 entries — oldest dropped when full.

use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use tracing::{info, warn};

const MAX_ENTRIES: usize = 10_000;

pub struct OfflineBuffer {
    path: PathBuf,
}

impl OfflineBuffer {
    pub fn new(dir: String) -> Self {
        let dir_path = PathBuf::from(&dir);
        if let Err(e) = fs::create_dir_all(&dir_path) {
            warn!(path = %dir, error = %e, "Failed to create buffer directory");
        }
        Self {
            path: dir_path.join("pending_batches.ndjson"),
        }
    }

    /// Append a serialised batch to the buffer file.
    pub fn write_batch(&self, payload: &Value) {
        let line = match serde_json::to_string(payload) {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "Failed to serialise batch for buffer");
                return;
            }
        };

        // Check entry count — drop oldest if at cap
        let current_count = self.count_entries();
        if current_count >= MAX_ENTRIES {
            self.drop_oldest(1);
            warn!("Offline buffer full ({MAX_ENTRIES} entries), dropped oldest entry");
        }

        let mut file = match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            Ok(f) => f,
            Err(e) => {
                warn!(error = %e, "Failed to open buffer file for writing");
                return;
            }
        };

        if let Err(e) = writeln!(file, "{}", line) {
            warn!(error = %e, "Failed to write batch to buffer");
        }
    }

    /// Read up to `limit` pending entries (oldest first).
    pub fn read_pending(&self, limit: usize) -> Vec<String> {
        let file = match fs::File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        BufReader::new(file)
            .lines()
            .take(limit)
            .filter_map(|line| line.ok())
            .filter(|line| !line.trim().is_empty())
            .collect()
    }

    /// Remove the first `count` entries from the buffer file.
    pub fn acknowledge(&self, count: usize) {
        let file = match fs::File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return,
        };

        let remaining: Vec<String> = BufReader::new(file)
            .lines()
            .filter_map(|l| l.ok())
            .skip(count)
            .collect();

        if remaining.is_empty() {
            let _ = fs::remove_file(&self.path);
        } else if let Ok(mut f) = fs::File::create(&self.path) {
            for line in &remaining {
                let _ = writeln!(f, "{}", line);
            }
        }
    }

    fn count_entries(&self) -> usize {
        let file = match fs::File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return 0,
        };
        BufReader::new(file).lines().count()
    }

    fn drop_oldest(&self, count: usize) {
        self.acknowledge(count);
    }
}
