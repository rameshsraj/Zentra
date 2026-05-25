//! sFlow v5 (RFC 3176) parser — sampled packet and counter samples.
//!
//! sFlow uses XDR encoding (big-endian, 4-byte aligned).
//! We parse sample_type=1 (flow_sample) and extract the raw packet header
//! to reconstruct src/dst IP and port tuples.

use std::net::{Ipv4Addr, Ipv6Addr};
use chrono::Utc;
use super::flow_aggregator::FlowRecord;

const SFLOW_V5: u32 = 5;
const SAMPLE_FLOW: u32 = 1;
const SAMPLE_COUNTER: u32 = 2;
const ENTERPRISE_STANDARD: u32 = 0;

const FLOW_DATA_RAW_HEADER: u32 = 1;
const HEADER_PROTO_ETHERNET: u32 = 1;
const HEADER_PROTO_IPV4: u32 = 11;
const HEADER_PROTO_IPV6: u32 = 12;

pub fn parse(buf: &[u8], exporter_ip: &str) -> Vec<FlowRecord> {
    if buf.len() < 28 { return vec![]; }

    let version = xdr_u32(buf, 0);
    if version != SFLOW_V5 { return vec![]; }

    // Address type: 1=IPv4, 2=IPv6
    let addr_type = xdr_u32(buf, 4);
    let (agent_addr, hdr_skip) = if addr_type == 1 {
        (Ipv4Addr::new(buf[8], buf[9], buf[10], buf[11]).to_string(), 12usize)
    } else if addr_type == 2 {
        let bytes: [u8; 16] = buf[8..24].try_into().unwrap_or([0u8; 16]);
        (Ipv6Addr::from(bytes).to_string(), 24usize)
    } else {
        return vec![];
    };

    let _sub_agent_id = xdr_u32(buf, hdr_skip);
    let _seq_number = xdr_u32(buf, hdr_skip + 4);
    let _sys_uptime = xdr_u32(buf, hdr_skip + 8);
    let num_samples = xdr_u32(buf, hdr_skip + 12) as usize;

    let mut pos = hdr_skip + 16;
    let mut flows = Vec::new();

    for _ in 0..num_samples {
        if pos + 8 > buf.len() { break; }

        let sample_type_raw = xdr_u32(buf, pos);
        let sample_len = xdr_u32(buf, pos + 4) as usize;
        pos += 8;

        if pos + sample_len > buf.len() { break; }
        let sample_data = &buf[pos..pos + sample_len];
        pos += sample_len;

        let enterprise = sample_type_raw >> 12;
        let sample_type = sample_type_raw & 0xFFF;

        if enterprise != ENTERPRISE_STANDARD { continue; }

        if sample_type == SAMPLE_FLOW {
            if let Some(rec) = parse_flow_sample(sample_data, exporter_ip) {
                flows.push(rec);
            }
        }
        // SAMPLE_COUNTER: interface counters — can be stored separately, skip for now
    }

    flows
}

fn parse_flow_sample(data: &[u8], exporter_ip: &str) -> Option<FlowRecord> {
    if data.len() < 32 { return None; }

    let _sequence_num = xdr_u32(data, 0);
    let _source_id = xdr_u32(data, 4);
    let sampling_rate = xdr_u32(data, 8).max(1) as u64;
    let _sample_pool = xdr_u32(data, 12);
    let _drops = xdr_u32(data, 16);
    let _input_if = xdr_u32(data, 20);
    let _output_if = xdr_u32(data, 24);
    let num_flow_records = xdr_u32(data, 28) as usize;

    let mut pos = 32;
    let mut src_ip = String::from("0.0.0.0");
    let mut dst_ip = String::from("0.0.0.0");
    let mut src_port = 0u16;
    let mut dst_port = 0u16;
    let mut protocol = 0u8;
    let mut frame_len_bytes = 0u64;

    for _ in 0..num_flow_records {
        if pos + 8 > data.len() { break; }
        let flow_type = xdr_u32(data, pos);
        let flow_len = xdr_u32(data, pos + 4) as usize;
        pos += 8;

        if pos + flow_len > data.len() { break; }
        let fdata = &data[pos..pos + flow_len];
        pos += flow_len;

        let enterprise = flow_type >> 12;
        let ftype = flow_type & 0xFFF;

        if enterprise == ENTERPRISE_STANDARD && ftype == FLOW_DATA_RAW_HEADER {
            if fdata.len() < 16 { continue; }
            let header_proto = xdr_u32(fdata, 0);
            frame_len_bytes = xdr_u32(fdata, 4) as u64;
            let stripped = xdr_u32(fdata, 8) as usize;
            let header_len = xdr_u32(fdata, 12) as usize;

            if fdata.len() < 16 + header_len { continue; }
            let raw_header = &fdata[16..16 + header_len];

            match header_proto {
                HEADER_PROTO_ETHERNET => {
                    // Ethernet frame: 14-byte header (dst 6, src 6, ethertype 2)
                    if raw_header.len() >= 34 {
                        let ethertype = u16::from_be_bytes([raw_header[12], raw_header[13]]);
                        if ethertype == 0x0800 {
                            parse_ipv4_header(&raw_header[14..], &mut src_ip, &mut dst_ip, &mut src_port, &mut dst_port, &mut protocol);
                        } else if ethertype == 0x86DD {
                            parse_ipv6_header(&raw_header[14..], &mut src_ip, &mut dst_ip, &mut src_port, &mut dst_port, &mut protocol);
                        }
                    }
                }
                HEADER_PROTO_IPV4 => {
                    parse_ipv4_header(raw_header, &mut src_ip, &mut dst_ip, &mut src_port, &mut dst_port, &mut protocol);
                }
                HEADER_PROTO_IPV6 => {
                    parse_ipv6_header(raw_header, &mut src_ip, &mut dst_ip, &mut src_port, &mut dst_port, &mut protocol);
                }
                _ => {}
            }
        }
    }

    // sFlow gives sampled packets; scale by sampling_rate for approximate totals
    let bytes = frame_len_bytes * sampling_rate;
    let packets = sampling_rate;

    Some(FlowRecord {
        received_at: Utc::now(),
        src_ip,
        dst_ip,
        src_port,
        dst_port,
        protocol,
        packets,
        bytes,
        duration_ms: 0,
        tcp_flags: None,
        tos: None,
        src_as: None,
        dst_as: None,
        src_prefix_len: None,
        dst_prefix_len: None,
        next_hop: None,
        flow_version: 50, // sFlow v5 marker
        exporter_ip: exporter_ip.to_string(),
        extra: serde_json::json!({ "sampling_rate": sampling_rate }),
    })
}

fn parse_ipv4_header(data: &[u8], src_ip: &mut String, dst_ip: &mut String, src_port: &mut u16, dst_port: &mut u16, protocol: &mut u8) {
    if data.len() < 20 { return; }
    let ihl = ((data[0] & 0x0F) as usize) * 4;
    *protocol = data[9];
    *src_ip = Ipv4Addr::new(data[12], data[13], data[14], data[15]).to_string();
    *dst_ip = Ipv4Addr::new(data[16], data[17], data[18], data[19]).to_string();
    if data.len() >= ihl + 4 {
        *src_port = u16::from_be_bytes([data[ihl], data[ihl + 1]]);
        *dst_port = u16::from_be_bytes([data[ihl + 2], data[ihl + 3]]);
    }
}

fn parse_ipv6_header(data: &[u8], src_ip: &mut String, dst_ip: &mut String, src_port: &mut u16, dst_port: &mut u16, protocol: &mut u8) {
    if data.len() < 40 { return; }
    *protocol = data[6];
    let src: [u8; 16] = data[8..24].try_into().unwrap_or([0u8; 16]);
    let dst: [u8; 16] = data[24..40].try_into().unwrap_or([0u8; 16]);
    *src_ip = Ipv6Addr::from(src).to_string();
    *dst_ip = Ipv6Addr::from(dst).to_string();
    if data.len() >= 44 {
        *src_port = u16::from_be_bytes([data[40], data[41]]);
        *dst_port = u16::from_be_bytes([data[42], data[43]]);
    }
}

fn xdr_u32(data: &[u8], offset: usize) -> u32 {
    if offset + 4 > data.len() { return 0; }
    u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}
