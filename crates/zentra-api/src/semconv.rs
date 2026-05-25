//! OTel P05: Semantic Convention Enrichment.
//!
//! Extracts well-known OTel semantic convention attributes from span
//! attribute maps and returns them as typed fields for indexed storage.

use serde_json::Value;

/// Extracted semantic convention fields from span attributes.
#[derive(Debug, Default)]
pub struct SemconvExtracted {
    pub http_method: Option<String>,
    pub http_status_code: Option<i32>,
    pub http_route: Option<String>,
    pub db_system: Option<String>,
    pub db_name: Option<String>,
    pub db_operation: Option<String>,
    pub rpc_system: Option<String>,
    pub rpc_service: Option<String>,
    pub rpc_method: Option<String>,
    pub messaging_system: Option<String>,
    pub peer_service: Option<String>,
}

/// Extract semconv fields from an OTLP attribute map (already converted to
/// a flat JSON object by `otlp_attrs_to_map`).
pub fn extract(attrs: &Value) -> SemconvExtracted {
    let mut out = SemconvExtracted::default();

    let obj = match attrs.as_object() {
        Some(o) => o,
        None => return out,
    };

    // HTTP — support both stable (http.request.method) and legacy (http.method)
    out.http_method = get_str(obj, "http.request.method")
        .or_else(|| get_str(obj, "http.method"));
    out.http_status_code = get_i32(obj, "http.response.status_code")
        .or_else(|| get_i32(obj, "http.status_code"));
    out.http_route = get_str(obj, "http.route")
        .or_else(|| get_str(obj, "http.target"));

    // Database
    out.db_system = get_str(obj, "db.system");
    out.db_name = get_str(obj, "db.name");
    out.db_operation = get_str(obj, "db.operation");

    // RPC
    out.rpc_system = get_str(obj, "rpc.system");
    out.rpc_service = get_str(obj, "rpc.service");
    out.rpc_method = get_str(obj, "rpc.method");

    // Messaging
    out.messaging_system = get_str(obj, "messaging.system");

    // Peer
    out.peer_service = get_str(obj, "peer.service");

    out
}

fn get_str(obj: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    obj.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn get_i32(obj: &serde_json::Map<String, Value>, key: &str) -> Option<i32> {
    obj.get(key).and_then(|v| {
        v.as_i64().map(|n| n as i32)
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_http() {
        let attrs = json!({"http.method": "GET", "http.status_code": "200", "http.route": "/api/users"});
        let ex = extract(&attrs);
        assert_eq!(ex.http_method.as_deref(), Some("GET"));
        assert_eq!(ex.http_status_code, Some(200));
        assert_eq!(ex.http_route.as_deref(), Some("/api/users"));
    }

    #[test]
    fn test_extract_stable_http() {
        let attrs = json!({"http.request.method": "POST", "http.response.status_code": 201});
        let ex = extract(&attrs);
        assert_eq!(ex.http_method.as_deref(), Some("POST"));
        assert_eq!(ex.http_status_code, Some(201));
    }

    #[test]
    fn test_extract_db() {
        let attrs = json!({"db.system": "postgresql", "db.name": "orders", "db.operation": "SELECT"});
        let ex = extract(&attrs);
        assert_eq!(ex.db_system.as_deref(), Some("postgresql"));
        assert_eq!(ex.db_name.as_deref(), Some("orders"));
    }

    #[test]
    fn test_extract_empty() {
        let attrs = json!({});
        let ex = extract(&attrs);
        assert!(ex.http_method.is_none());
        assert!(ex.db_system.is_none());
    }
}
