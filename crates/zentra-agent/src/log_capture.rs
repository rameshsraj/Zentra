//! In-memory ring buffer of recent tracing log events.
//!
//! Provides a `tracing_subscriber::Layer` that captures formatted log lines
//! into a bounded shared buffer. The heartbeat loop drains this buffer every
//! 30s and POSTs the lines to the Zentra server, where they are surfaced in
//! the install wizard's live-tail panel (and optionally persisted via OTLP).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// Maximum log lines retained in the in-memory buffer.
/// Anything older is dropped when new lines arrive.
const BUFFER_CAP: usize = 200;

/// A single structured log line ready to be shipped to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLogLine {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
}

/// Shared, thread-safe ring buffer.
#[derive(Clone)]
pub struct LogBuffer {
    inner: Arc<Mutex<VecDeque<AgentLogLine>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(BUFFER_CAP))),
        }
    }

    fn push(&self, line: AgentLogLine) {
        let mut buf = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if buf.len() >= BUFFER_CAP {
            buf.pop_front();
        }
        buf.push_back(line);
    }

    /// Drain all currently buffered lines and return them.
    /// Called by the heartbeat loop right before POSTing.
    pub fn drain(&self) -> Vec<AgentLogLine> {
        let mut buf = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        buf.drain(..).collect()
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Visitor that extracts the `message` field (and any other string fields)
/// from a tracing event into a single rendered string.
struct MessageVisitor {
    out: String,
}

impl MessageVisitor {
    fn new() -> Self {
        Self { out: String::new() }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            // The default tracing message uses Debug; render without quotes.
            let _ = write!(self.out, "{:?}", value);
        } else {
            if !self.out.is_empty() {
                let _ = write!(self.out, " ");
            }
            let _ = write!(self.out, "{}={:?}", field.name(), value);
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            if !self.out.is_empty() {
                let _ = write!(self.out, " ");
            }
            let _ = write!(self.out, "{}", value);
        } else {
            if !self.out.is_empty() {
                let _ = write!(self.out, " ");
            }
            let _ = write!(self.out, "{}={}", field.name(), value);
        }
    }
}

/// `tracing_subscriber::Layer` that pushes every event into a `LogBuffer`.
pub struct CaptureLayer {
    buffer: LogBuffer,
}

impl CaptureLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }
}

impl<S> Layer<S> for CaptureLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut visitor = MessageVisitor::new();
        event.record(&mut visitor);
        let message = visitor.out.trim().to_string();
        if message.is_empty() {
            return;
        }
        self.buffer.push(AgentLogLine {
            timestamp: Utc::now(),
            level: meta.level().to_string(),
            target: meta.target().to_string(),
            message,
        });
    }
}
