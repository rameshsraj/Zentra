use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// File analysis module — hashing, YARA matching, type detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReport {
    pub filename: String,
    pub file_size: u64,
    pub mime_type: String,
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
    pub verdict: FileVerdict,
    pub yara_matches: Vec<YaraMatch>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FileVerdict {
    Clean,
    Suspicious,
    Malicious,
    Unknown,
}

impl std::fmt::Display for FileVerdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Clean => write!(f, "clean"),
            Self::Suspicious => write!(f, "suspicious"),
            Self::Malicious => write!(f, "malicious"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YaraMatch {
    pub rule: String,
    pub namespace: String,
    pub tags: Vec<String>,
    pub strings: Vec<String>,
}

/// Compute MD5, SHA1, SHA256 for a byte buffer
pub fn compute_hashes(data: &[u8]) -> (String, String, String) {
    use sha2::{Sha256, Digest as Sha2Digest};
    use md5;

    let md5_hash = format!("{:x}", md5::compute(data));

    // sha1
    let mut sha1_hasher = sha1::Sha1::new();
    sha1::Digest::update(&mut sha1_hasher, data);
    let sha1_hash = format!("{:x}", sha1::Digest::finalize(sha1_hasher));

    let mut sha256_hasher = Sha256::new();
    sha2::Digest::update(&mut sha256_hasher, data);
    let sha256_hash = format!("{:x}", sha2::Digest::finalize(sha256_hasher));

    (md5_hash, sha1_hash, sha256_hash)
}

/// Simple YARA rule matching (placeholder — real impl would use yara-rust crate)
pub fn match_yara_rules(data: &[u8], rules: &[CompiledYaraRule]) -> Vec<YaraMatch> {
    let mut matches = Vec::new();
    for rule in rules {
        for pattern in &rule.patterns {
            if data.windows(pattern.len()).any(|w| w == pattern.as_bytes()) {
                matches.push(YaraMatch {
                    rule: rule.name.clone(),
                    namespace: rule.namespace.clone(),
                    tags: rule.tags.clone(),
                    strings: vec![pattern.clone()],
                });
                break;
            }
        }
    }
    matches
}

pub struct CompiledYaraRule {
    pub name: String,
    pub namespace: String,
    pub tags: Vec<String>,
    pub patterns: Vec<String>,
}

/// Detect MIME type from magic bytes
pub fn detect_mime(data: &[u8]) -> &'static str {
    if data.len() < 4 { return "application/octet-stream"; }
    match &data[..4] {
        [0x89, 0x50, 0x4E, 0x47] => "image/png",
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [0x25, 0x50, 0x44, 0x46] => "application/pdf",
        [0x50, 0x4B, 0x03, 0x04] => "application/zip",
        [0x7F, 0x45, 0x4C, 0x46] => "application/x-elf",
        [0x4D, 0x5A, ..] => "application/x-dosexec",
        _ => "application/octet-stream",
    }
}
