use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Parsed connection log entry (Zeek conn.log format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionLog {
    pub uid: String,
    pub timestamp: DateTime<Utc>,
    pub src_ip: String,
    pub src_port: u16,
    pub dest_ip: String,
    pub dest_port: u16,
    pub proto: String,
    pub service: Option<String>,
    pub duration: Option<f64>,
    pub bytes_orig: i64,
    pub bytes_resp: i64,
    pub pkts_orig: i64,
    pub pkts_resp: i64,
    pub conn_state: String,
    pub community_id: Option<String>,
}

/// Parse Zeek conn.log TSV line
pub fn parse_conn_log_line(line: &str) -> Option<ConnectionLog> {
    if line.starts_with('#') { return None; }
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() < 15 { return None; }

    let ts_secs: f64 = fields[0].parse().ok()?;
    let secs = ts_secs as i64;
    let nsecs = ((ts_secs - secs as f64) * 1_000_000_000.0) as u32;
    let timestamp = DateTime::from_timestamp(secs, nsecs)?;

    Some(ConnectionLog {
        timestamp,
        uid: fields[1].to_string(),
        src_ip: fields[2].to_string(),
        src_port: fields[3].parse().unwrap_or(0),
        dest_ip: fields[4].to_string(),
        dest_port: fields[5].parse().unwrap_or(0),
        proto: fields[6].to_string(),
        service: if fields[7] == "-" { None } else { Some(fields[7].to_string()) },
        duration: fields[8].parse().ok(),
        bytes_orig: fields[9].parse().unwrap_or(0),
        bytes_resp: fields[10].parse().unwrap_or(0),
        pkts_orig: fields[13].parse().unwrap_or(0),
        pkts_resp: fields[14].parse().unwrap_or(0),
        conn_state: fields.get(11).unwrap_or(&"").to_string(),
        community_id: None,
    })
}

/// Classify connection state
pub fn classify_conn_state(state: &str) -> &'static str {
    match state {
        "S0" => "Connection attempt, no reply",
        "S1" => "Connection established, not terminated",
        "SF" => "Normal establishment and termination",
        "REJ" => "Connection rejected",
        "S2" => "Connection established, originator closed",
        "S3" => "Connection established, responder closed",
        "RSTO" => "Connection reset by originator",
        "RSTR" => "Connection reset by responder",
        "RSTOS0" => "Originator SYN, responder RST",
        "RSTRH" => "Responder RST, originator ack",
        "SH" => "Originator SYN then FIN, no SYN-ACK",
        "SHR" => "Responder SYN-ACK then FIN, no SYN",
        "OTH" => "Midstream traffic, no SYN/FIN",
        _ => "Unknown",
    }
}
