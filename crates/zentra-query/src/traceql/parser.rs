//! TraceQL parser: parses tokens into an AST.

use super::lexer::{Token, TokenKind};

#[derive(Debug, Clone)]
pub struct Query {
    pub conditions: Vec<Condition>,
    pub logic: Vec<LogicOp>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogicOp {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum Condition {
    AttributeMatch {
        attribute: String,
        op: CompareOp,
        value: String,
    },
    DurationMatch {
        op: CompareOp,
        duration_ms: i64,
    },
    StatusMatch {
        status: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompareOp {
    Eq,
    Neq,
    RegexMatch,
    RegexNoMatch,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug)]
pub struct ParseError {
    pub position: usize,
    pub message: String,
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() { self.pos += 1; }
        t
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<(), ParseError> {
        let t = self.peek();
        if std::mem::discriminant(&t.kind) == std::mem::discriminant(kind) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError {
                position: t.position,
                message: format!("expected {:?}, got {:?}", kind, t.kind),
            })
        }
    }

    fn parse_query(&mut self) -> Result<Query, ParseError> {
        self.expect(&TokenKind::LBrace)?;

        let mut conditions = Vec::new();
        let mut logic = Vec::new();

        conditions.push(self.parse_condition()?);

        loop {
            match &self.peek().kind {
                TokenKind::And => {
                    self.advance();
                    logic.push(LogicOp::And);
                    conditions.push(self.parse_condition()?);
                }
                TokenKind::Or => {
                    self.advance();
                    logic.push(LogicOp::Or);
                    conditions.push(self.parse_condition()?);
                }
                _ => break,
            }
        }

        self.expect(&TokenKind::RBrace)?;

        Ok(Query { conditions, logic })
    }

    fn parse_condition(&mut self) -> Result<Condition, ParseError> {
        let t = self.peek().clone();
        match &t.kind {
            TokenKind::Ident(name) => {
                let name = name.clone();

                if name == "duration" {
                    self.advance();
                    let op = self.parse_compare_op()?;
                    let dur = match &self.peek().kind {
                        TokenKind::Duration(ms) => {
                            let ms = *ms;
                            self.advance();
                            ms
                        }
                        _ => return Err(ParseError {
                            position: self.peek().position,
                            message: "expected duration value (e.g. 500ms, 1s)".into(),
                        }),
                    };
                    Ok(Condition::DurationMatch { op, duration_ms: dur })
                } else if name == "status" {
                    self.advance();
                    self.parse_compare_op()?; // consume =
                    match &self.peek().kind {
                        TokenKind::StatusLit(s) => {
                            let s = s.clone();
                            self.advance();
                            Ok(Condition::StatusMatch { status: s })
                        }
                        TokenKind::StringLit(s) => {
                            let s = s.clone();
                            self.advance();
                            Ok(Condition::StatusMatch { status: s })
                        }
                        _ => Err(ParseError {
                            position: self.peek().position,
                            message: "expected status value (ok, error, unset)".into(),
                        }),
                    }
                } else {
                    self.advance();
                    let op = self.parse_compare_op()?;
                    let value = match &self.peek().kind {
                        TokenKind::StringLit(s) => {
                            let s = s.clone();
                            self.advance();
                            s
                        }
                        TokenKind::StatusLit(s) => {
                            let s = s.clone();
                            self.advance();
                            s
                        }
                        _ => return Err(ParseError {
                            position: self.peek().position,
                            message: "expected string value".into(),
                        }),
                    };
                    Ok(Condition::AttributeMatch { attribute: name, op, value })
                }
            }
            _ => Err(ParseError {
                position: t.position,
                message: format!("expected attribute name, got {:?}", t.kind),
            }),
        }
    }

    fn parse_compare_op(&mut self) -> Result<CompareOp, ParseError> {
        let t = self.peek().clone();
        let op = match t.kind {
            TokenKind::Eq => CompareOp::Eq,
            TokenKind::Neq => CompareOp::Neq,
            TokenKind::RegexMatch => CompareOp::RegexMatch,
            TokenKind::RegexNoMatch => CompareOp::RegexNoMatch,
            TokenKind::Gt => CompareOp::Gt,
            TokenKind::Gte => CompareOp::Gte,
            TokenKind::Lt => CompareOp::Lt,
            TokenKind::Lte => CompareOp::Lte,
            _ => return Err(ParseError {
                position: t.position,
                message: format!("expected comparison operator, got {:?}", t.kind),
            }),
        };
        self.advance();
        Ok(op)
    }
}

pub fn parse(tokens: &[Token]) -> Result<Query, ParseError> {
    let mut parser = Parser::new(tokens);
    let query = parser.parse_query()?;

    if !matches!(parser.peek().kind, TokenKind::Eof) {
        return Err(ParseError {
            position: parser.peek().position,
            message: "unexpected tokens after query".into(),
        });
    }

    Ok(query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traceql::lexer::tokenize;

    #[test]
    fn test_parse_status() {
        let tokens = tokenize("{status=error}").unwrap();
        let q = parse(&tokens).unwrap();
        assert_eq!(q.conditions.len(), 1);
        assert!(matches!(&q.conditions[0], Condition::StatusMatch { status } if status == "error"));
    }

    #[test]
    fn test_parse_duration() {
        let tokens = tokenize("{duration > 1s}").unwrap();
        let q = parse(&tokens).unwrap();
        assert!(matches!(&q.conditions[0], Condition::DurationMatch { op: CompareOp::Gt, duration_ms: 1000 }));
    }

    #[test]
    fn test_parse_compound() {
        let tokens = tokenize("{service.name=\"order\" && duration > 500ms}").unwrap();
        let q = parse(&tokens).unwrap();
        assert_eq!(q.conditions.len(), 2);
        assert_eq!(q.logic.len(), 1);
        assert_eq!(q.logic[0], LogicOp::And);
    }
}
