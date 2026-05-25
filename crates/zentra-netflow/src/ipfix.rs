//! IPFIX (IP Flow Information Export — RFC 7011) parser.
//!
//! IPFIX is the IETF standardisation of NetFlow v9. The wire format is
//! structurally identical to v9 (set ID 2 = template set, 3 = options template,
//! ≥256 = data set) with a slightly different header and variable-length fields.
//! We reuse the v9 template cache and field mappings.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::net::Ipv4Addr;
use chrono::Utc;
use super::flow_aggregator::FlowRecord;
use super::netflow_v9::{TemplateCache, TemplateField, Template};

pub fn parse(buf: &[u8], exporter_ip: &str, cache: &TemplateCache) -> Vec<FlowRecord> {
    if buf.len() < 16 { return vec![]; }

    let version = u16::from_be_bytes([buf[0], buf[1]]);
    if version != 10 { return vec![]; }

    let _length = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    let _export_time = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let _seq = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let _observation_domain = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);

    let mut pos = 16usize;
    let mut flows = Vec::new();

    while pos + 4 <= buf.len() {
        let set_id = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let set_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;

        if set_len < 4 || pos + set_len > buf.len() { break; }

        let set_data = &buf[pos + 4..pos + set_len];

        match set_id {
            2 => parse_ipfix_template_set(set_data, exporter_ip, cache),
            3 => { /* options template — skip */ }
            id if id >= 256 => {
                let tmpl = cache.read().get(&(exporter_ip.to_string(), set_id)).cloned();
                if let Some(tmpl) = tmpl {
                    let mut off = 0;
                    while off + tmpl.record_len <= set_data.len() {
                        if let Some(rec) = parse_ipfix_data_record(&set_data[off..off + tmpl.record_len], &tmpl, exporter_ip) {
                            flows.push(rec);
                        }
                        off += tmpl.record_len;
                    }
                }
            }
            _ => {}
        }

        pos += set_len;
    }

    flows
}

fn parse_ipfix_template_set(data: &[u8], exporter_ip: &str, cache: &TemplateCache) {
    let mut pos = 0;
    while pos + 4 <= data.len() {
        let template_id = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let field_count = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if template_id < 256 { continue; }
        if pos + field_count * 4 > data.len() { break; }

        let mut fields = Vec::with_capacity(field_count);
        let mut record_len = 0usize;

        for _ in 0..field_count {
            // In IPFIX, enterprise bit (bit 15) may be set; mask it
            let raw_type = u16::from_be_bytes([data[pos], data[pos + 1]]);
            let type_id = raw_type & 0x7FFF;
            let length = u16::from_be_bytes([data[pos + 2], data[pos + 3]]);
            let enterprise = raw_type & 0x8000 != 0;

            pos += 4;
            if enterprise && pos + 4 <= data.len() {
                // Skip 4-byte enterprise number
                pos += 4;
            }

            // Variable-length fields (length == 65535) not yet supported
            if length == 65535 { continue; }

            record_len += length as usize;
            fields.push(TemplateField { type_id, length });
        }

        cache.write().insert((exporter_ip.to_string(), template_id), Template { template_id, fields, record_len });
    }
}

fn parse_ipfix_data_record(data: &[u8], tmpl: &Template, exporter_ip: &str) -> Option<FlowRecord> {
    // Field IDs in IPFIX match NetFlow v9 IANA Information Elements (same numeric IDs)
    // Reuse v9 parsing logic directly
    super::netflow_v9::parse_via_template(data, tmpl, exporter_ip, 10)
}
