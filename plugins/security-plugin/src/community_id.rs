use sha2::{Sha256, Digest};

/// Community ID v1 hash for correlating network events across tools.
/// Spec: https://github.com/corelight/community-id-spec
///
/// Produces a deterministic, direction-independent hash from:
/// (seed, src_ip, dst_ip, proto, src_port, dst_port)
pub fn compute_community_id(
    src_ip: &str,
    dst_ip: &str,
    proto: u8,
    src_port: u16,
    dst_port: u16,
    seed: u16,
) -> String {
    // Determine ordering: lower IP/port first for direction independence
    let (ordered_src_ip, ordered_dst_ip, ordered_src_port, ordered_dst_port) =
        if src_ip < dst_ip || (src_ip == dst_ip && src_port < dst_port) {
            (src_ip, dst_ip, src_port, dst_port)
        } else {
            (dst_ip, src_ip, dst_port, src_port)
        };

    let mut hasher = Sha256::new();
    hasher.update(seed.to_be_bytes());
    hasher.update(ordered_src_ip.as_bytes());
    hasher.update(ordered_dst_ip.as_bytes());
    hasher.update(proto.to_be_bytes());
    hasher.update([0u8]); // padding
    hasher.update(ordered_src_port.to_be_bytes());
    hasher.update(ordered_dst_port.to_be_bytes());

    let hash = hasher.finalize();
    let b64 = base64_encode(&hash[..20]); // Use first 20 bytes
    format!("1:{}", b64)
}

/// Protocol number from name
pub fn proto_number(name: &str) -> u8 {
    match name.to_lowercase().as_str() {
        "tcp" => 6,
        "udp" => 17,
        "icmp" => 1,
        "icmp6" | "icmpv6" => 58,
        "sctp" => 132,
        _ => 0,
    }
}

/// Simple base64 encode (no dependency needed)
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() { data[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if i + 1 < data.len() {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if i + 2 < data.len() {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        i += 3;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_community_id_basic() {
        let cid = compute_community_id("10.0.0.1", "10.0.0.2", 6, 12345, 80, 0);
        assert!(cid.starts_with("1:"));
        assert!(!cid.is_empty());
    }

    #[test]
    fn test_direction_independence() {
        let a = compute_community_id("10.0.0.1", "10.0.0.2", 6, 12345, 80, 0);
        let b = compute_community_id("10.0.0.2", "10.0.0.1", 6, 80, 12345, 0);
        assert_eq!(a, b);
    }
}
