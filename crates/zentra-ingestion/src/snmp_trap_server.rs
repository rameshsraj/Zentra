//! SNMP Trap UDP receiver — parses v1/v2c/v3 traps and INFORMs, persists to DB.
//!
//! Binds to UDP 162 (configurable). Uses a minimal hand-rolled BER/ASN.1 decoder
//! sufficient for SNMPv1 trap PDUs and SNMPv2c trap / inform PDUs.
//! SNMPv3 messages are stored raw; decryption/auth is out of scope for v1.

use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use sqlx::PgPool;
use chrono::Utc;
use serde_json::{json, Value};

const MAX_PDU: usize = 65535;

pub struct SnmpTrapServer {
    bind_addr: String,
    pool: PgPool,
}

impl SnmpTrapServer {
    pub fn new(bind_addr: impl Into<String>, pool: PgPool) -> Self {
        Self { bind_addr: bind_addr.into(), pool }
    }

    pub async fn run(self) {
        let sock = match UdpSocket::bind(&self.bind_addr).await {
            Ok(s) => s,
            Err(e) => {
                error!(addr = %self.bind_addr, error = %e, "Failed to bind SNMP trap UDP socket");
                return;
            }
        };
        info!(addr = %self.bind_addr, "SNMP trap receiver listening");

        let mut buf = vec![0u8; MAX_PDU];
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((len, peer)) => {
                    let data = buf[..len].to_vec();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        handle_trap(data, peer, pool).await;
                    });
                }
                Err(e) => {
                    error!(error = %e, "SNMP trap UDP recv error");
                }
            }
        }
    }
}

async fn handle_trap(data: Vec<u8>, peer: SocketAddr, pool: PgPool) {
    match parse_snmp_message(&data) {
        Ok(msg) => {
            debug!(peer = %peer, version = %msg.version, oid = ?msg.enterprise_oid, "SNMP trap received");
            if let Err(e) = persist_trap(&pool, peer, msg).await {
                error!(peer = %peer, error = %e, "Failed to persist SNMP trap");
            }
        }
        Err(e) => {
            warn!(peer = %peer, error = %e, bytes = data.len(), "Failed to parse SNMP trap PDU");
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// ASN.1 / BER decoder (minimal, trap-only subset)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct SnmpMessage {
    version: String,
    community: Option<String>,
    enterprise_oid: Option<String>,
    trap_type: Option<String>,
    varbinds: Value,
}

fn parse_snmp_message(buf: &[u8]) -> Result<SnmpMessage, String> {
    let mut pos = 0usize;

    // Outer SEQUENCE
    let (_tag, outer_len) = read_tlv_header(buf, &mut pos)?;
    if pos + outer_len > buf.len() {
        return Err("truncated message".into());
    }
    let end = pos + outer_len;

    // Version INTEGER
    let version_int = read_integer(buf, &mut pos)? as u8;
    let version = match version_int {
        0 => "v1",
        1 => "v2c",
        3 => "v3",
        _ => "unknown",
    }.to_string();

    // Community STRING (v1/v2c) or msgGlobalData (v3, treated as opaque)
    let community = if version_int <= 1 {
        Some(read_octet_string_utf8(buf, &mut pos).unwrap_or_default())
    } else {
        skip_tlv(buf, &mut pos).ok();
        None
    };

    if version_int == 3 {
        // SNMPv3: store raw varbinds as hex; full decryption not implemented
        let hex = hex_encode(&buf[pos..end]);
        return Ok(SnmpMessage {
            version,
            community: None,
            enterprise_oid: None,
            trap_type: Some("v3_raw".into()),
            varbinds: json!({ "raw_hex": hex }),
        });
    }

    // PDU type tag
    if pos >= end {
        return Err("missing PDU".into());
    }
    let pdu_tag = buf[pos];
    pos += 1;
    let pdu_len = read_length(buf, &mut pos)?;
    let pdu_end = pos + pdu_len;

    let (enterprise_oid, trap_type, varbinds) = match pdu_tag {
        0xA4 => parse_v1_trap_pdu(buf, pos, pdu_end)?,
        0xA7 => parse_v2_inform_pdu(buf, pos, pdu_end)?,
        0xA2 | 0xA6 => parse_v2_trap_pdu(buf, pos, pdu_end)?, // A6 = trap-v2
        _ => (None, Some(format!("pdu_tag_0x{:02X}", pdu_tag)), json!([])),
    };

    Ok(SnmpMessage { version, community, enterprise_oid, trap_type, varbinds })
}

/// SNMPv1 Trap-PDU (tag 0xA4)
fn parse_v1_trap_pdu(buf: &[u8], mut pos: usize, end: usize) -> Result<(Option<String>, Option<String>, Value), String> {
    let enterprise_oid = read_oid(buf, &mut pos).ok();
    // agent-addr (4 bytes)
    skip_tlv(buf, &mut pos).ok();
    // generic-trap INTEGER
    let generic = read_integer(buf, &mut pos).unwrap_or(-1);
    // specific-trap INTEGER
    let specific = read_integer(buf, &mut pos).unwrap_or(0);
    // time-stamp
    skip_tlv(buf, &mut pos).ok();

    let trap_type = generic_trap_name(generic, specific);
    let varbinds = parse_varbind_list(buf, pos, end);

    Ok((enterprise_oid, Some(trap_type), varbinds))
}

/// SNMPv2c trap PDU (tag 0xA6) — sysUpTime + snmpTrapOID + varbinds
fn parse_v2_trap_pdu(buf: &[u8], mut pos: usize, end: usize) -> Result<(Option<String>, Option<String>, Value), String> {
    // request-id, error-status, error-index
    skip_tlv(buf, &mut pos).ok();
    skip_tlv(buf, &mut pos).ok();
    skip_tlv(buf, &mut pos).ok();

    // VarBindList
    let varbinds_val = parse_varbind_list(buf, pos, end);

    // Extract snmpTrapOID.0 from varbind array
    let trap_oid = extract_trap_oid_from_varbinds(&varbinds_val);

    Ok((trap_oid.clone(), trap_oid.map(|s| format!("v2c_trap:{}", s)), varbinds_val))
}

fn parse_v2_inform_pdu(buf: &[u8], pos: usize, end: usize) -> Result<(Option<String>, Option<String>, Value), String> {
    let (oid, _, vb) = parse_v2_trap_pdu(buf, pos, end)?;
    Ok((oid, Some("inform".into()), vb))
}

fn parse_varbind_list(buf: &[u8], mut pos: usize, end: usize) -> Value {
    // Outer SEQUENCE of SEQUENCE
    if pos >= end { return json!([]); }
    if buf[pos] != 0x30 { return json!([]); }
    pos += 1;
    let list_len = match read_length(buf, &mut pos) {
        Ok(l) => l,
        Err(_) => return json!([]),
    };
    let list_end = (pos + list_len).min(end);

    let mut binds = Vec::new();
    while pos < list_end {
        // Each varbind is SEQUENCE { OID, value }
        if buf.get(pos) != Some(&0x30) { break; }
        pos += 1;
        let seq_len = match read_length(buf, &mut pos) { Ok(l) => l, Err(_) => break };
        let seq_end = (pos + seq_len).min(list_end);

        let oid = read_oid(buf, &mut pos).unwrap_or_else(|_| "".into());
        let val = if pos < seq_end {
            decode_value(buf, &mut pos)
        } else {
            json!(null)
        };
        binds.push(json!({ "oid": oid, "value": val }));
        pos = seq_end;
    }
    json!(binds)
}

fn extract_trap_oid_from_varbinds(vb: &Value) -> Option<String> {
    // snmpTrapOID.0 is 1.3.6.1.6.3.1.1.4.1.0
    let arr = vb.as_array()?;
    for item in arr {
        let oid = item.get("oid")?.as_str()?;
        if oid == "1.3.6.1.6.3.1.1.4.1.0" {
            return item.get("value").and_then(|v| v.as_str()).map(|s| s.to_string());
        }
    }
    arr.get(1).and_then(|v| v.get("value")).and_then(|v| v.as_str()).map(|s| s.to_string())
}

// ──────────────────────────────────────────────────────────────────────────────
// BER primitive decoders
// ──────────────────────────────────────────────────────────────────────────────

fn read_tlv_header(buf: &[u8], pos: &mut usize) -> Result<(u8, usize), String> {
    if *pos >= buf.len() { return Err("EOF at TLV header".into()); }
    let tag = buf[*pos]; *pos += 1;
    let len = read_length(buf, pos)?;
    Ok((tag, len))
}

fn read_length(buf: &[u8], pos: &mut usize) -> Result<usize, String> {
    if *pos >= buf.len() { return Err("EOF at length".into()); }
    let first = buf[*pos] as usize; *pos += 1;
    if first < 0x80 {
        return Ok(first);
    }
    let n_bytes = first & 0x7F;
    if n_bytes == 0 || n_bytes > 4 { return Err(format!("unsupported length encoding: {} bytes", n_bytes)); }
    let mut len = 0usize;
    for _ in 0..n_bytes {
        if *pos >= buf.len() { return Err("EOF in multi-byte length".into()); }
        len = (len << 8) | (buf[*pos] as usize);
        *pos += 1;
    }
    Ok(len)
}

fn read_integer(buf: &[u8], pos: &mut usize) -> Result<i64, String> {
    let (tag, len) = read_tlv_header(buf, pos)?;
    if tag != 0x02 { return Err(format!("expected INTEGER(0x02), got 0x{:02X}", tag)); }
    if len == 0 { return Ok(0); }
    if *pos + len > buf.len() { return Err("truncated INTEGER".into()); }
    let mut val = if buf[*pos] & 0x80 != 0 { -1i64 } else { 0i64 };
    for &b in &buf[*pos..*pos + len] {
        val = (val << 8) | (b as i64);
    }
    *pos += len;
    Ok(val)
}

fn read_octet_string_utf8(buf: &[u8], pos: &mut usize) -> Result<String, String> {
    let (tag, len) = read_tlv_header(buf, pos)?;
    if tag != 0x04 { return Err(format!("expected OCTET STRING(0x04), got 0x{:02X}", tag)); }
    if *pos + len > buf.len() { return Err("truncated OCTET STRING".into()); }
    let s = String::from_utf8_lossy(&buf[*pos..*pos + len]).into_owned();
    *pos += len;
    Ok(s)
}

fn read_oid(buf: &[u8], pos: &mut usize) -> Result<String, String> {
    let (tag, len) = read_tlv_header(buf, pos)?;
    if tag != 0x06 { return Err(format!("expected OID(0x06), got 0x{:02X}", tag)); }
    if len == 0 { return Ok("0.0".into()); }
    if *pos + len > buf.len() { return Err("truncated OID".into()); }
    let raw = &buf[*pos..*pos + len];
    *pos += len;

    let mut components = Vec::new();
    components.push(raw[0] / 40);
    components.push(raw[0] % 40);
    let mut acc = 0u64;
    for &b in &raw[1..] {
        acc = (acc << 7) | ((b & 0x7F) as u64);
        if b & 0x80 == 0 {
            components.push(acc as u8);
            acc = 0;
        }
    }
    Ok(components.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("."))
}

fn skip_tlv(buf: &[u8], pos: &mut usize) -> Result<(), String> {
    let (_, len) = read_tlv_header(buf, pos)?;
    if *pos + len > buf.len() { return Err("truncated TLV skip".into()); }
    *pos += len;
    Ok(())
}

fn decode_value(buf: &[u8], pos: &mut usize) -> Value {
    if *pos >= buf.len() { return json!(null); }
    let tag = buf[*pos];
    match tag {
        0x02 => json!(read_integer(buf, pos).unwrap_or(0)),
        0x04 => json!(read_octet_string_utf8(buf, pos).unwrap_or_default()),
        0x05 => { skip_tlv(buf, pos).ok(); json!(null) }
        0x06 => json!(read_oid(buf, pos).unwrap_or_default()),
        0x40 => { // IP Address
            if let Ok((_, len)) = read_tlv_header(buf, pos) {
                if *pos + 4 <= buf.len() && len >= 4 {
                    let ip = format!("{}.{}.{}.{}", buf[*pos], buf[*pos+1], buf[*pos+2], buf[*pos+3]);
                    *pos += len;
                    return json!(ip);
                }
                *pos += len;
            }
            json!(null)
        }
        0x41 | 0x42 | 0x43 | 0x46 => { // Counter32/Gauge32/TimeTicks/Counter64
            *pos += 1;
            let len = read_length(buf, pos).unwrap_or(0);
            let mut val = 0u64;
            for i in 0..len.min(8) {
                val = (val << 8) | (buf.get(*pos + i).copied().unwrap_or(0) as u64);
            }
            *pos += len;
            json!(val)
        }
        _ => {
            skip_tlv(buf, pos).ok();
            json!(format!("tag_0x{:02X}", tag))
        }
    }
}

fn generic_trap_name(generic: i64, specific: i64) -> String {
    match generic {
        0 => "coldStart".into(),
        1 => "warmStart".into(),
        2 => "linkDown".into(),
        3 => "linkUp".into(),
        4 => "authenticationFailure".into(),
        5 => "egpNeighborLoss".into(),
        6 => format!("enterpriseSpecific({})", specific),
        _ => format!("generic({})", generic),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ──────────────────────────────────────────────────────────────────────────────
// DB persistence
// ──────────────────────────────────────────────────────────────────────────────

async fn persist_trap(pool: &PgPool, peer: SocketAddr, msg: SnmpMessage) -> Result<(), sqlx::Error> {
    let peer_ip = peer.ip().to_string();
    let peer_port = peer.port() as i32;

    // Classify severity from varbinds or trap type
    let severity = classify_severity(&msg);

    sqlx::query(
        r#"
        INSERT INTO snmp_traps
            (id, received_at, source_ip, source_port, version, community,
             enterprise_oid, trap_type, varbinds, severity)
        VALUES ($1, $2, $3::inet, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(Utc::now())
    .bind(peer_ip)
    .bind(peer_port)
    .bind(msg.version)
    .bind(msg.community)
    .bind(msg.enterprise_oid)
    .bind(msg.trap_type)
    .bind(msg.varbinds)
    .bind(severity)
    .execute(pool)
    .await?;

    Ok(())
}

fn classify_severity(msg: &SnmpMessage) -> String {
    let trap = msg.trap_type.as_deref().unwrap_or("");
    if trap.contains("linkDown") || trap.contains("authenticationFailure") {
        return "warning".into();
    }
    if trap.contains("coldStart") || trap.contains("warmStart") {
        return "info".into();
    }
    // Check varbind OIDs for well-known severity indicators
    if let Some(arr) = msg.varbinds.as_array() {
        for vb in arr {
            if let Some(oid) = vb.get("oid").and_then(|v| v.as_str()) {
                // Cisco criticalAlarms: 1.3.6.1.4.1.9.9.41.1.2.3.1.5
                if oid.contains("1.3.6.1.4.1.9.9.41") {
                    return "critical".into();
                }
            }
        }
    }
    "info".into()
}
