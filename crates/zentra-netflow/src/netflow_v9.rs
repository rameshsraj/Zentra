//! NetFlow v9 — template-based, variable record format (RFC 3954).
//!
//! v9 requires caching templates per (exporter_ip, template_id).
//! Templates arrive in template flowsets (flowset_id == 0).
//! Data records reference previously seen templates.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use parking_lot::RwLock;
use std::sync::Arc;
use chrono::Utc;
use super::flow_aggregator::FlowRecord;

// Field type IDs (RFC 3954 §8)
const IN_BYTES: u16 = 1;
const IN_PKTS: u16 = 2;
const PROTOCOL: u16 = 4;
const SRC_TOS: u16 = 5;
const TCP_FLAGS: u16 = 6;
const L4_SRC_PORT: u16 = 7;
const IPV4_SRC_ADDR: u16 = 8;
const SRC_MASK: u16 = 9;
const INPUT_SNMP: u16 = 10;
const L4_DST_PORT: u16 = 11;
const IPV4_DST_ADDR: u16 = 12;
const DST_MASK: u16 = 13;
const OUTPUT_SNMP: u16 = 14;
const IPV4_NEXT_HOP: u16 = 15;
const SRC_AS: u16 = 16;
const DST_AS: u16 = 17;
const LAST_SWITCHED: u16 = 21;
const FIRST_SWITCHED: u16 = 22;
const OUT_BYTES: u16 = 23;
const OUT_PKTS: u16 = 24;

#[derive(Clone, Debug)]
pub struct TemplateField {
    pub type_id: u16,
    pub length: u16,
}

#[derive(Clone, Debug)]
pub struct Template {
    pub template_id: u16,
    pub fields: Vec<TemplateField>,
    pub record_len: usize,
}

pub type TemplateCache = Arc<RwLock<HashMap<(String, u16), Template>>>;

pub fn new_template_cache() -> TemplateCache {
    Arc::new(RwLock::new(HashMap::new()))
}

pub fn parse(buf: &[u8], exporter_ip: &str, cache: &TemplateCache) -> Vec<FlowRecord> {
    if buf.len() < 20 { return vec![]; }

    let version = u16::from_be_bytes([buf[0], buf[1]]);
    if version != 9 { return vec![]; }

    let count = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    let sys_uptime = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let _unix_secs = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let _seq = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let _source_id = u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);

    let mut pos = 20usize;
    let mut flows = Vec::new();
    let mut flowsets_parsed = 0usize;

    while pos + 4 <= buf.len() && flowsets_parsed < count + 32 {
        let flowset_id = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let flowset_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;

        if flowset_len < 4 || pos + flowset_len > buf.len() { break; }

        let flowset_data = &buf[pos + 4..pos + flowset_len];

        match flowset_id {
            0 => {
                // Template flowset
                parse_template_flowset(flowset_data, exporter_ip, cache);
            }
            1 => {
                // Options template — skip for now
            }
            id if id >= 256 => {
                // Data flowset
                let template_id = flowset_id;
                let tmpl = cache.read().get(&(exporter_ip.to_string(), template_id)).cloned();
                if let Some(tmpl) = tmpl {
                    let mut off = 0;
                    while off + tmpl.record_len <= flowset_data.len() {
                        if let Some(rec) = parse_data_record(&flowset_data[off..], &tmpl, exporter_ip, sys_uptime, 9) {
                            flows.push(rec);
                        }
                        off += tmpl.record_len;
                    }
                }
            }
            _ => {}
        }

        pos += flowset_len;
        flowsets_parsed += 1;
    }

    flows
}

fn parse_template_flowset(data: &[u8], exporter_ip: &str, cache: &TemplateCache) {
    let mut pos = 0;
    while pos + 4 <= data.len() {
        let template_id = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let field_count = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if template_id < 256 { continue; } // invalid
        if pos + field_count * 4 > data.len() { break; }

        let mut fields = Vec::with_capacity(field_count);
        let mut record_len = 0usize;

        for _ in 0..field_count {
            let type_id = u16::from_be_bytes([data[pos], data[pos + 1]]);
            let length = u16::from_be_bytes([data[pos + 2], data[pos + 3]]);
            record_len += length as usize;
            fields.push(TemplateField { type_id, length });
            pos += 4;
        }

        let tmpl = Template { template_id, fields, record_len };
        cache.write().insert((exporter_ip.to_string(), template_id), tmpl);
    }
}

/// Public entry point used by IPFIX parser (shares same field semantics)
pub fn parse_via_template(data: &[u8], tmpl: &Template, exporter_ip: &str, version: u16) -> Option<FlowRecord> {
    parse_data_record(data, tmpl, exporter_ip, 0, version)
}

fn parse_data_record(data: &[u8], tmpl: &Template, exporter_ip: &str, _sys_uptime: u32, flow_version: u16) -> Option<FlowRecord> {
    let mut pos = 0;
    let mut src_ip = String::from("0.0.0.0");
    let mut dst_ip = String::from("0.0.0.0");
    let mut src_port = 0u16;
    let mut dst_port = 0u16;
    let mut protocol = 0u8;
    let mut bytes = 0u64;
    let mut packets = 0u64;
    let mut tcp_flags = None;
    let mut tos = None;
    let mut src_as = None;
    let mut dst_as = None;
    let mut src_mask = None;
    let mut dst_mask = None;
    let mut next_hop: Option<String> = None;
    let mut first_ms = 0u64;
    let mut last_ms = 0u64;

    for field in &tmpl.fields {
        let end = pos + field.length as usize;
        if end > data.len() { break; }
        let fdata = &data[pos..end];

        match field.type_id {
            IN_BYTES | OUT_BYTES => bytes += read_u64(fdata),
            IN_PKTS | OUT_PKTS => packets += read_u64(fdata),
            PROTOCOL => protocol = fdata[0],
            SRC_TOS => tos = Some(fdata[0]),
            TCP_FLAGS => tcp_flags = Some(fdata[0]),
            L4_SRC_PORT => src_port = read_u16(fdata),
            L4_DST_PORT => dst_port = read_u16(fdata),
            IPV4_SRC_ADDR if fdata.len() == 4 => {
                src_ip = Ipv4Addr::new(fdata[0], fdata[1], fdata[2], fdata[3]).to_string();
            }
            IPV4_DST_ADDR if fdata.len() == 4 => {
                dst_ip = Ipv4Addr::new(fdata[0], fdata[1], fdata[2], fdata[3]).to_string();
            }
            IPV4_NEXT_HOP if fdata.len() == 4 => {
                let nh = Ipv4Addr::new(fdata[0], fdata[1], fdata[2], fdata[3]);
                if !nh.is_unspecified() { next_hop = Some(nh.to_string()); }
            }
            SRC_AS => src_as = Some(read_u64(fdata) as u32),
            DST_AS => dst_as = Some(read_u64(fdata) as u32),
            SRC_MASK => src_mask = Some(fdata[0]),
            DST_MASK => dst_mask = Some(fdata[0]),
            FIRST_SWITCHED => first_ms = read_u64(fdata),
            LAST_SWITCHED => last_ms = read_u64(fdata),
            _ => {}
        }
        pos = end;
    }

    let duration_ms = last_ms.saturating_sub(first_ms);

    Some(FlowRecord {
        received_at: Utc::now(),
        src_ip,
        dst_ip,
        src_port,
        dst_port,
        protocol,
        packets,
        bytes,
        duration_ms,
        tcp_flags,
        tos,
        src_as,
        dst_as,
        src_prefix_len: src_mask,
        dst_prefix_len: dst_mask,
        next_hop,
        flow_version,
        exporter_ip: exporter_ip.to_string(),
        extra: serde_json::Value::Null,
    })
}

fn read_u16(data: &[u8]) -> u16 {
    match data.len() {
        1 => data[0] as u16,
        2 => u16::from_be_bytes([data[0], data[1]]),
        _ => 0,
    }
}

fn read_u64(data: &[u8]) -> u64 {
    let mut v = 0u64;
    for &b in data.iter().take(8) {
        v = (v << 8) | (b as u64);
    }
    v
}
