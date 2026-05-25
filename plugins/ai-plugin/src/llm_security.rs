use regex::Regex;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// Prompt Injection Detector
// ═══════════════════════════════════════════════════════════════════════════

static INJECTION_PATTERNS: &[&str] = &[
    r"(?i)(ignore|disregard|forget)\s+(all\s+)?(previous|prior|above)\s+(instructions|rules|context)",
    r"(?i)(you\s+are\s+now|act\s+as|pretend\s+to\s+be)\s+(?:DAN|evil|unrestricted)",
    r"(?i)system\s*:\s*override",
    r"(?i)<!--\s*system\s*:",
    r"(?i)\[INST\]|\[/INST\]|<\|im_start\|>|<\|im_end\|>",
    r"(?i)(reveal|show|display|print)\s+(your|the)\s+(system\s+prompt|instructions|rules)",
    r"(?i)do\s+anything\s+now",
];

#[derive(Debug, Serialize, Deserialize)]
pub struct InjectionResult {
    pub is_injection: bool,
    pub score: f64,
    pub matched_patterns: Vec<String>,
}

pub fn detect_prompt_injection(text: &str) -> InjectionResult {
    let mut matched = Vec::new();
    for pat in INJECTION_PATTERNS {
        if let Ok(re) = Regex::new(pat) {
            if re.is_match(text) {
                matched.push(pat.to_string());
            }
        }
    }
    let score = if matched.is_empty() { 0.0 } else { (matched.len() as f64 / INJECTION_PATTERNS.len() as f64).min(1.0) * 0.5 + 0.5 };
    InjectionResult { is_injection: !matched.is_empty(), score, matched_patterns: matched }
}

// ═══════════════════════════════════════════════════════════════════════════
// PII Scanner
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize)]
pub struct PiiMatch {
    pub pii_type: String,
    pub count: usize,
}

static PII_PATTERNS: &[(&str, &str)] = &[
    ("ssn", r"\b\d{3}-\d{2}-\d{4}\b"),
    ("credit_card", r"\b(?:\d{4}[- ]?){3}\d{4}\b"),
    ("phone", r"\b(?:\+1[- ]?)?\(?\d{3}\)?[- ]?\d{3}[- ]?\d{4}\b"),
    ("email", r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b"),
];

pub fn scan_pii(text: &str) -> Vec<PiiMatch> {
    let mut results = Vec::new();
    for (label, pat) in PII_PATTERNS {
        if let Ok(re) = Regex::new(pat) {
            let count = re.find_iter(text).count();
            if count > 0 {
                results.push(PiiMatch { pii_type: label.to_string(), count });
            }
        }
    }
    results
}

// ═══════════════════════════════════════════════════════════════════════════
// Toxicity Scorer (keyword-based)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize)]
pub struct ToxicityResult {
    pub score: f64,
    pub severity: String,
    pub categories: Vec<String>,
}

static TOXICITY_KEYWORDS: &[(&str, &[&str])] = &[
    ("violence", &["kill", "murder", "weapon", "bomb", "attack", "harm"]),
    ("hate_speech", &["slur", "racist", "bigot", "supremacist"]),
    ("self_harm", &["suicide", "self-harm", "cut myself"]),
    ("illegal", &["hack into", "exploit", "steal", "counterfeit"]),
];

pub fn score_toxicity(text: &str) -> ToxicityResult {
    let lower = text.to_lowercase();
    let mut cats = Vec::new();
    let mut hits = 0u32;
    let mut total = 0u32;

    for (cat, keywords) in TOXICITY_KEYWORDS {
        total += keywords.len() as u32;
        for kw in *keywords {
            if lower.contains(kw) {
                hits += 1;
                if !cats.contains(&cat.to_string()) { cats.push(cat.to_string()); }
            }
        }
    }

    let score = if total > 0 { hits as f64 / total as f64 } else { 0.0 };
    let severity = if score > 0.5 { "high" } else if score > 0.2 { "medium" } else if score > 0.0 { "low" } else { "none" };

    ToxicityResult { score, severity: severity.into(), categories: cats }
}

// ═══════════════════════════════════════════════════════════════════════════
// Hallucination Detector (simple overlap scoring)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize)]
pub struct HallucinationResult {
    pub score: f64,
    pub is_hallucination: bool,
    pub overlap_ratio: f64,
}

pub fn detect_hallucination(response: &str, context: &str) -> HallucinationResult {
    if context.is_empty() || response.is_empty() {
        return HallucinationResult { score: 0.0, is_hallucination: false, overlap_ratio: 1.0 };
    }

    let response_words: std::collections::HashSet<&str> = response.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 3)
        .collect();
    let context_words: std::collections::HashSet<&str> = context.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| w.len() > 3)
        .collect();

    if response_words.is_empty() {
        return HallucinationResult { score: 0.0, is_hallucination: false, overlap_ratio: 1.0 };
    }

    let overlap = response_words.intersection(&context_words).count();
    let overlap_ratio = overlap as f64 / response_words.len() as f64;
    let score = 1.0 - overlap_ratio;

    HallucinationResult { score, is_hallucination: score > 0.7, overlap_ratio }
}
