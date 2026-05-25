//! In-memory flow aggregation and TopN computation before DB write.

use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use parking_lot::Mutex;
use std::sync::Arc;
use sqlx::PgPool;
use tracing::{info, error};

#[derive(Debug, Clone)]
pub struct FlowRecord {
    pub received_at: DateTime<Utc>,
    pub src_ip: String,
    pub dst_ip: String,
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
    pub packets: u64,
    pub bytes: u64,
    pub duration_ms: u64,
    pub tcp_flags: Option<u8>,
    pub tos: Option<u8>,
    pub src_as: Option<u32>,
    pub dst_as: Option<u32>,
    pub src_prefix_len: Option<u8>,
    pub dst_prefix_len: Option<u8>,
    pub next_hop: Option<String>,
    pub flow_version: u16,
    pub exporter_ip: String,
    pub extra: Value,
}

#[derive(Debug, Clone, Default)]
struct FlowBucket {
    packets: u64,
    bytes: u64,
    flows: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FlowKey {
    src_ip: String,
    dst_ip: String,
    src_port: u16,
    dst_port: u16,
    protocol: u8,
    exporter_ip: String,
}

pub struct FlowAggregator {
    pool: PgPool,
    pending: Arc<Mutex<Vec<FlowRecord>>>,
    aggregated: Arc<Mutex<HashMap<FlowKey, FlowBucket>>>,
    last_eviction: Arc<Mutex<chrono::DateTime<Utc>>>,
}

impl FlowAggregator {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            pending: Arc::new(Mutex::new(Vec::new())),
            aggregated: Arc::new(Mutex::new(HashMap::new())),
            last_eviction: Arc::new(Mutex::new(Utc::now())),
        }
    }

    pub fn push(&self, mut record: FlowRecord, exporter_ip: &str) {
        record.exporter_ip = exporter_ip.to_string();

        let key = FlowKey {
            src_ip: record.src_ip.clone(),
            dst_ip: record.dst_ip.clone(),
            src_port: record.src_port,
            dst_port: record.dst_port,
            protocol: record.protocol,
            exporter_ip: exporter_ip.to_string(),
        };

        {
            let mut agg = self.aggregated.lock();
            // Evict stale entries every 5 minutes to prevent unbounded growth
            let mut last = self.last_eviction.lock();
            let now = Utc::now();
            if (now - *last).num_minutes() >= 5 {
                agg.retain(|_, v| v.flows > 0);
                // Hard cap: if still over 50k entries, clear to prevent OOM
                if agg.len() > 50_000 {
                    tracing::warn!(count = agg.len(), "Flow aggregator exceeded 50k entries, clearing");
                    agg.clear();
                }
                *last = now;
            }
            let bucket = agg.entry(key).or_default();
            bucket.packets += record.packets;
            bucket.bytes += record.bytes;
            bucket.flows += 1;
        }

        self.pending.lock().push(record);
    }

    /// Flush pending flows to DB every N seconds
    pub async fn run_flush_loop(self: Arc<Self>, interval_secs: u64) {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            self.flush().await;
        }
    }

    async fn flush(&self) {
        let records: Vec<FlowRecord> = {
            let mut lock = self.pending.lock();
            std::mem::take(&mut *lock)
        };

        if records.is_empty() { return; }

        info!(count = records.len(), "Flushing flow records to DB");

        for record in &records {
            let protocol_name = protocol_name(record.protocol);
            if let Err(e) = sqlx::query!(
                r#"
                INSERT INTO netflow_records
                    (id, received_at, exporter_ip, src_ip, dst_ip, src_port, dst_port,
                     protocol, protocol_name, packets, bytes, duration_ms,
                     tcp_flags, tos, src_as, dst_as, flow_version, extra)
                VALUES
                    (uuid_generate_v4(), $1, $2::text::inet, $3::text::inet, $4::text::inet, $5, $6,
                     $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
                ON CONFLICT DO NOTHING
                "#,
                record.received_at,
                record.exporter_ip,
                record.src_ip,
                record.dst_ip,
                record.src_port as i32,
                record.dst_port as i32,
                record.protocol as i32,
                protocol_name,
                record.packets as i64,
                record.bytes as i64,
                record.duration_ms as i64,
                record.tcp_flags.map(|f| f as i32),
                record.tos.map(|t| t as i32),
                record.src_as.map(|a| a as i64),
                record.dst_as.map(|a| a as i64),
                record.flow_version as i32,
                record.extra,
            )
            .execute(&self.pool)
            .await {
                error!(error = %e, "Flow DB insert failed");
            }
        }
    }
}

fn protocol_name(proto: u8) -> &'static str {
    match proto {
        1 => "ICMP", 6 => "TCP", 17 => "UDP",
        47 => "GRE", 50 => "ESP", 51 => "AH",
        58 => "ICMPv6", 89 => "OSPF", 132 => "SCTP",
        _ => "OTHER",
    }
}
