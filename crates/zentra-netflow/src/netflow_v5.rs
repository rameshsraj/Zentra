//! NetFlow v5 — fixed 48-byte record format, big-endian, UDP.
//!
//! Header: 24 bytes. Each record: 48 bytes. Max 30 records per packet.

use std::net::Ipv4Addr;
use chrono::Utc;
use super::flow_aggregator::FlowRecord;

const HEADER_LEN: usize = 24;
const RECORD_LEN: usize = 48;

pub fn parse(buf: &[u8]) -> Vec<FlowRecord> {
    if buf.len() < HEADER_LEN { return vec![]; }

    let version = u16::from_be_bytes([buf[0], buf[1]]);
    if version != 5 { return vec![]; }

    let count = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    let sys_uptime_ms = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]) as u64;
    // unix_secs / unix_nsecs at bytes 8-15 (not used — we use wall clock)
    let _engine_type = buf[16];
    let _engine_id = buf[17];
    // sampling interval at bytes 18-19

    let max_records = (buf.len() - HEADER_LEN) / RECORD_LEN;
    let records_to_parse = count.min(max_records);

    let now = Utc::now();
    let mut flows = Vec::with_capacity(records_to_parse);

    for i in 0..records_to_parse {
        let off = HEADER_LEN + i * RECORD_LEN;
        let r = &buf[off..off + RECORD_LEN];

        let src_ip = Ipv4Addr::new(r[0], r[1], r[2], r[3]);
        let dst_ip = Ipv4Addr::new(r[4], r[5], r[6], r[7]);
        let next_hop = Ipv4Addr::new(r[8], r[9], r[10], r[11]);
        let _input_if = u16::from_be_bytes([r[12], r[13]]);
        let _output_if = u16::from_be_bytes([r[14], r[15]]);
        let packets = u32::from_be_bytes([r[16], r[17], r[18], r[19]]) as u64;
        let bytes = u32::from_be_bytes([r[20], r[21], r[22], r[23]]) as u64;
        let first_ms = u32::from_be_bytes([r[24], r[25], r[26], r[27]]) as u64;
        let last_ms = u32::from_be_bytes([r[28], r[29], r[30], r[31]]) as u64;
        let src_port = u16::from_be_bytes([r[32], r[33]]);
        let dst_port = u16::from_be_bytes([r[34], r[35]]);
        // r[36] = pad
        let tcp_flags = r[37];
        let protocol = r[38];
        let tos = r[39];
        let src_as = u16::from_be_bytes([r[40], r[41]]) as u32;
        let dst_as = u16::from_be_bytes([r[42], r[43]]) as u32;
        let src_mask = r[44];
        let dst_mask = r[45];

        let duration_ms = last_ms.saturating_sub(first_ms);

        flows.push(FlowRecord {
            received_at: now,
            src_ip: src_ip.to_string(),
            dst_ip: dst_ip.to_string(),
            src_port,
            dst_port,
            protocol,
            packets,
            bytes,
            duration_ms,
            tcp_flags: Some(tcp_flags),
            tos: Some(tos),
            src_as: Some(src_as),
            dst_as: Some(dst_as),
            src_prefix_len: Some(src_mask),
            dst_prefix_len: Some(dst_mask),
            next_hop: if next_hop.is_unspecified() { None } else { Some(next_hop.to_string()) },
            flow_version: 5,
            exporter_ip: String::new(), // filled by caller
            extra: serde_json::Value::Null,
        });
    }

    flows
}
