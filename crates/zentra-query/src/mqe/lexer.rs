//! MQE-Lite lexer: tokenizes metric query expressions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TokenKind {
    Ident(String),       // metric names, function names, label keys
    StringLit(String),   // "value"
    Number(f64),
    Duration(i64),       // seconds, parsed from 5m, 1h, 30s etc.
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Eq,                  // =
    Neq,                 // !=
    RegexMatch,          // =~
    RegexNoMatch,        // !~
    Plus,
    Minus,
    Star,
    Slash,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub position: usize,
}

#[derive(Debug)]
pub struct LexError {
    pub position: usize,
    pub message: String,
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        // Skip whitespace
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        let pos = i;

        match ch {
            '(' => { tokens.push(Token { kind: TokenKind::LParen, position: pos }); i += 1; }
            ')' => { tokens.push(Token { kind: TokenKind::RParen, position: pos }); i += 1; }
            '{' => { tokens.push(Token { kind: TokenKind::LBrace, position: pos }); i += 1; }
            '}' => { tokens.push(Token { kind: TokenKind::RBrace, position: pos }); i += 1; }
            '[' => { tokens.push(Token { kind: TokenKind::LBracket, position: pos }); i += 1; }
            ']' => { tokens.push(Token { kind: TokenKind::RBracket, position: pos }); i += 1; }
            ',' => { tokens.push(Token { kind: TokenKind::Comma, position: pos }); i += 1; }
            '+' => { tokens.push(Token { kind: TokenKind::Plus, position: pos }); i += 1; }
            '-' => { tokens.push(Token { kind: TokenKind::Minus, position: pos }); i += 1; }
            '*' => { tokens.push(Token { kind: TokenKind::Star, position: pos }); i += 1; }
            '/' => { tokens.push(Token { kind: TokenKind::Slash, position: pos }); i += 1; }
            '=' => {
                if i + 1 < chars.len() && chars[i + 1] == '~' {
                    tokens.push(Token { kind: TokenKind::RegexMatch, position: pos });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Eq, position: pos });
                    i += 1;
                }
            }
            '!' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token { kind: TokenKind::Neq, position: pos });
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1] == '~' {
                    tokens.push(Token { kind: TokenKind::RegexNoMatch, position: pos });
                    i += 2;
                } else {
                    return Err(LexError { position: pos, message: "unexpected '!'".into() });
                }
            }
            '"' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() { i += 1; }
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(LexError { position: pos, message: "unterminated string".into() });
                }
                let s: String = chars[start..i].iter().collect();
                tokens.push(Token { kind: TokenKind::StringLit(s), position: pos });
                i += 1; // skip closing "
            }
            _ if ch.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().collect();
                // Check if followed by duration suffix
                if i < chars.len() && matches!(chars[i], 's' | 'm' | 'h' | 'd') {
                    let suffix = chars[i];
                    i += 1;
                    let num: f64 = num_str.parse().map_err(|_| LexError { position: pos, message: "invalid number".into() })?;
                    let seconds = match suffix {
                        's' => num as i64,
                        'm' => (num * 60.0) as i64,
                        'h' => (num * 3600.0) as i64,
                        'd' => (num * 86400.0) as i64,
                        _ => unreachable!(),
                    };
                    tokens.push(Token { kind: TokenKind::Duration(seconds), position: pos });
                } else {
                    let num: f64 = num_str.parse().map_err(|_| LexError { position: pos, message: "invalid number".into() })?;
                    tokens.push(Token { kind: TokenKind::Number(num), position: pos });
                }
            }
            _ if ch.is_ascii_alphanumeric() || ch == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                tokens.push(Token { kind: TokenKind::Ident(ident), position: pos });
            }
            _ => {
                return Err(LexError { position: pos, message: format!("unexpected character '{}'", ch) });
            }
        }
    }

    tokens.push(Token { kind: TokenKind::Eof, position: input.len() });
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_metric() {
        let tokens = tokenize("cpu_usage_percent").unwrap();
        assert_eq!(tokens.len(), 2); // ident + eof
        assert!(matches!(&tokens[0].kind, TokenKind::Ident(s) if s == "cpu_usage_percent"));
    }

    #[test]
    fn test_function_with_label() {
        let tokens = tokenize("avg(cpu{host=\"web-01\"})").unwrap();
        assert!(tokens.len() > 5);
    }

    #[test]
    fn test_duration() {
        let tokens = tokenize("[5m]").unwrap();
        assert!(matches!(tokens[1].kind, TokenKind::Duration(300)));
    }
}
