//! MQE-Lite parser: parses tokens into an AST.

use super::lexer::{Token, TokenKind};

#[derive(Debug, Clone)]
pub enum Expr {
    /// A raw metric selector, e.g. `cpu_usage_percent{host="web-01"}`
    Selector {
        metric: String,
        matchers: Vec<LabelMatcher>,
    },
    /// An aggregation function call, e.g. `avg(cpu_usage_percent[5m])`
    FuncCall {
        func: AggFunc,
        inner: Box<Expr>,
        window_seconds: Option<i64>,
    },
    /// A binary math operation, e.g. `expr + expr`
    BinaryOp {
        left: Box<Expr>,
        op: MathOp,
        right: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
pub struct LabelMatcher {
    pub key: String,
    pub op: MatchOp,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatchOp {
    Eq,
    Neq,
    RegexMatch,
    RegexNoMatch,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AggFunc {
    Avg,
    Sum,
    Max,
    Min,
    Count,
    Rate,
    Increase,
    Delta,
}

impl AggFunc {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "avg" => Some(Self::Avg),
            "sum" => Some(Self::Sum),
            "max" => Some(Self::Max),
            "min" => Some(Self::Min),
            "count" => Some(Self::Count),
            "rate" => Some(Self::Rate),
            "increase" => Some(Self::Increase),
            "delta" => Some(Self::Delta),
            _ => None,
        }
    }

    pub fn as_sql(&self) -> &'static str {
        match self {
            Self::Avg => "AVG",
            Self::Sum => "SUM",
            Self::Max => "MAX",
            Self::Min => "MIN",
            Self::Count => "COUNT",
            Self::Rate => "rate", // special handling
            Self::Increase => "increase",
            Self::Delta => "delta",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MathOp {
    Add,
    Sub,
    Mul,
    Div,
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
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<&Token, ParseError> {
        let t = self.peek();
        if std::mem::discriminant(&t.kind) == std::mem::discriminant(kind) {
            Ok(self.advance())
        } else {
            Err(ParseError {
                position: t.position,
                message: format!("expected {:?}, got {:?}", kind, t.kind),
            })
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_primary()?;

        // Check for binary math op
        let t = self.peek();
        match &t.kind {
            TokenKind::Plus | TokenKind::Minus | TokenKind::Star | TokenKind::Slash => {
                let op = match &t.kind {
                    TokenKind::Plus => MathOp::Add,
                    TokenKind::Minus => MathOp::Sub,
                    TokenKind::Star => MathOp::Mul,
                    TokenKind::Slash => MathOp::Div,
                    _ => unreachable!(),
                };
                self.advance();
                let right = self.parse_primary()?;
                Ok(Expr::BinaryOp {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                })
            }
            _ => Ok(left),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let t = self.peek().clone();
        match &t.kind {
            TokenKind::Ident(name) => {
                let name = name.clone();
                // Check if it's a function call
                if let Some(func) = AggFunc::from_str(&name) {
                    self.advance(); // consume function name
                    self.expect(&TokenKind::LParen)?;
                    let inner = self.parse_primary()?;
                    // Optional window
                    let window = if matches!(self.peek().kind, TokenKind::LBracket) {
                        self.advance(); // [
                        let dur = match &self.peek().kind {
                            TokenKind::Duration(d) => {
                                let d = *d;
                                self.advance();
                                d
                            }
                            _ => return Err(ParseError {
                                position: self.peek().position,
                                message: "expected duration (e.g. 5m)".into(),
                            }),
                        };
                        self.expect(&TokenKind::RBracket)?;
                        Some(dur)
                    } else {
                        None
                    };
                    self.expect(&TokenKind::RParen)?;
                    Ok(Expr::FuncCall {
                        func,
                        inner: Box::new(inner),
                        window_seconds: window,
                    })
                } else {
                    // It's a metric selector
                    self.advance(); // consume metric name
                    let matchers = if matches!(self.peek().kind, TokenKind::LBrace) {
                        self.parse_label_matchers()?
                    } else {
                        vec![]
                    };
                    // Optional window (for bare selectors with window)
                    let window = if matches!(self.peek().kind, TokenKind::LBracket) {
                        self.advance();
                        let dur = match &self.peek().kind {
                            TokenKind::Duration(d) => {
                                let d = *d;
                                self.advance();
                                d
                            }
                            _ => return Err(ParseError {
                                position: self.peek().position,
                                message: "expected duration".into(),
                            }),
                        };
                        self.expect(&TokenKind::RBracket)?;
                        Some(dur)
                    } else {
                        None
                    };
                    let sel = Expr::Selector { metric: name, matchers };
                    if let Some(_w) = window {
                        // Bare selector with window — wrap in implicit identity
                        Ok(sel)
                    } else {
                        Ok(sel)
                    }
                }
            }
            _ => Err(ParseError {
                position: t.position,
                message: format!("expected metric name or function, got {:?}", t.kind),
            }),
        }
    }

    fn parse_label_matchers(&mut self) -> Result<Vec<LabelMatcher>, ParseError> {
        self.expect(&TokenKind::LBrace)?;
        let mut matchers = Vec::new();

        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            let key = match &self.peek().kind {
                TokenKind::Ident(k) => {
                    let k = k.clone();
                    self.advance();
                    k
                }
                _ => return Err(ParseError {
                    position: self.peek().position,
                    message: "expected label key".into(),
                }),
            };

            let op = match &self.peek().kind {
                TokenKind::Eq => { self.advance(); MatchOp::Eq }
                TokenKind::Neq => { self.advance(); MatchOp::Neq }
                TokenKind::RegexMatch => { self.advance(); MatchOp::RegexMatch }
                TokenKind::RegexNoMatch => { self.advance(); MatchOp::RegexNoMatch }
                _ => return Err(ParseError {
                    position: self.peek().position,
                    message: "expected comparison operator (=, !=, =~, !~)".into(),
                }),
            };

            let value = match &self.peek().kind {
                TokenKind::StringLit(s) => {
                    let s = s.clone();
                    self.advance();
                    s
                }
                _ => return Err(ParseError {
                    position: self.peek().position,
                    message: "expected string value".into(),
                }),
            };

            matchers.push(LabelMatcher { key, op, value });

            if matches!(self.peek().kind, TokenKind::Comma) {
                self.advance();
            }
        }

        self.expect(&TokenKind::RBrace)?;
        Ok(matchers)
    }
}

pub fn parse(tokens: &[Token]) -> Result<Expr, ParseError> {
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_expr()?;

    // Expect EOF
    if !matches!(parser.peek().kind, TokenKind::Eof) {
        return Err(ParseError {
            position: parser.peek().position,
            message: format!("unexpected token {:?} after expression", parser.peek().kind),
        });
    }

    Ok(expr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mqe::lexer::tokenize;

    #[test]
    fn test_parse_simple_metric() {
        let tokens = tokenize("cpu_usage_percent").unwrap();
        let expr = parse(&tokens).unwrap();
        assert!(matches!(expr, Expr::Selector { metric, .. } if metric == "cpu_usage_percent"));
    }

    #[test]
    fn test_parse_function_call() {
        let tokens = tokenize("avg(cpu_usage_percent{host=\"web-01\"})").unwrap();
        let expr = parse(&tokens).unwrap();
        assert!(matches!(expr, Expr::FuncCall { func: AggFunc::Avg, .. }));
    }

    #[test]
    fn test_parse_binary_op() {
        let tokens = tokenize("sum(errors) / sum(requests)").unwrap();
        let expr = parse(&tokens).unwrap();
        assert!(matches!(expr, Expr::BinaryOp { op: MathOp::Div, .. }));
    }

    #[test]
    fn test_parse_error() {
        let tokens = tokenize("avg({").unwrap();
        let result = parse(&tokens);
        assert!(result.is_err());
    }
}
