//! TraceQL lexer: tokenizes trace query expressions.

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    LBrace,
    RBrace,
    And,          // &&
    Or,           // ||
    Eq,           // =
    Neq,          // !=
    RegexMatch,   // =~
    RegexNoMatch, // !~
    Gt,           // >
    Gte,          // >=
    Lt,           // <
    Lte,          // <=
    Ident(String),
    StringLit(String),
    Duration(i64),    // milliseconds
    StatusLit(String), // ok, error, unset
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
        if ch.is_whitespace() { i += 1; continue; }

        let pos = i;

        match ch {
            '{' => { tokens.push(Token { kind: TokenKind::LBrace, position: pos }); i += 1; }
            '}' => { tokens.push(Token { kind: TokenKind::RBrace, position: pos }); i += 1; }
            '&' => {
                if i + 1 < chars.len() && chars[i + 1] == '&' {
                    tokens.push(Token { kind: TokenKind::And, position: pos });
                    i += 2;
                } else {
                    return Err(LexError { position: pos, message: "expected '&&'".into() });
                }
            }
            '|' => {
                if i + 1 < chars.len() && chars[i + 1] == '|' {
                    tokens.push(Token { kind: TokenKind::Or, position: pos });
                    i += 2;
                } else {
                    return Err(LexError { position: pos, message: "expected '||'".into() });
                }
            }
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
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token { kind: TokenKind::Gte, position: pos });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Gt, position: pos });
                    i += 1;
                }
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token { kind: TokenKind::Lte, position: pos });
                    i += 2;
                } else {
                    tokens.push(Token { kind: TokenKind::Lt, position: pos });
                    i += 1;
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
                i += 1;
            }
            _ if ch.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
                let num_str: String = chars[start..i].iter().collect();
                let num: i64 = num_str.parse().map_err(|_| LexError { position: pos, message: "invalid number".into() })?;

                // Check for duration suffix
                if i < chars.len() {
                    let ms = match chars[i] {
                        'h' => { i += 1; num * 3_600_000 }
                        'm' if i + 1 < chars.len() && chars[i + 1] == 's' => { i += 2; num }
                        'm' => { i += 1; num * 60_000 }
                        's' => { i += 1; num * 1_000 }
                        _ => {
                            return Err(LexError { position: pos, message: "expected duration suffix (ms, s, m, h)".into() });
                        }
                    };
                    tokens.push(Token { kind: TokenKind::Duration(ms), position: pos });
                } else {
                    return Err(LexError { position: pos, message: "expected duration suffix".into() });
                }
            }
            _ if ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();

                // Check for status keywords
                match ident.as_str() {
                    "ok" | "error" | "unset" => {
                        tokens.push(Token { kind: TokenKind::StatusLit(ident), position: pos });
                    }
                    _ => {
                        tokens.push(Token { kind: TokenKind::Ident(ident), position: pos });
                    }
                }
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
    fn test_simple_query() {
        let tokens = tokenize("{status=error}").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::LBrace));
        assert!(matches!(&tokens[1].kind, TokenKind::Ident(s) if s == "status"));
        assert!(matches!(tokens[2].kind, TokenKind::Eq));
        assert!(matches!(&tokens[3].kind, TokenKind::StatusLit(s) if s == "error"));
        assert!(matches!(tokens[4].kind, TokenKind::RBrace));
    }

    #[test]
    fn test_duration() {
        let tokens = tokenize("{duration > 500ms}").unwrap();
        assert!(matches!(tokens[2].kind, TokenKind::Gt));
        assert!(matches!(tokens[3].kind, TokenKind::Duration(500)));
    }

    #[test]
    fn test_compound() {
        let tokens = tokenize("{service.name=\"order\" && duration > 1s}").unwrap();
        assert!(tokens.len() > 5);
    }
}
