//! Turns a list of [`Token`]s into an executable [`Expr`].
//!
//! The [`Parser`] should roughly follow the structure from:
//! <https://github.com/jqlang/jq/blob/ea9e41f7587e2a515c9b7d28f3ab6ed00d30e5ce/src/parser.y>
//!
//! # Grammar
//!
//! Simplified jq-like grammar with implicit piping:
//!
//! ```text
//! Expr → Term ( ( '|' | ε ) Term )*
//! Term → FIELD '?'?
//!      | DOT
//!      | '[' INTEGER ']' '?'?
//!      | '[' ']'
//! ```
//!
//! `UPPERCASE` symbols denote terminals, and `ε` denotes end of input.

// NOTE: Please keep the grammar above up-to-date.

use super::lexer::{Token, TokenType};

pub struct Parser<I>
where
    I: Iterator<Item = Token>,
{
    tokens: std::iter::Peekable<I>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SegmentKind {
    Field(String),
    Index(u64),
    Each,
}

impl std::fmt::Display for SegmentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Field(name) => write!(f, ".{name}"),
            Self::Index(n) => write!(f, "[{n}]"),
            Self::Each => write!(f, "[]"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Segment {
    pub kind: SegmentKind,
    pub optional: bool,
}

impl std::fmt::Display for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind)?;
        if self.optional {
            write!(f, "?")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Identity,
    Path(Vec<Segment>),
    Pipe(Box<Self>, Box<Self>),
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity => write!(f, "."),
            Self::Path(segments) => {
                for segment in segments {
                    write!(f, "{segment}")?;
                }
                Ok(())
            }
            Self::Pipe(left, right) => write!(f, "{left} | {right}"),
        }
    }
}

// TODO(RR-3438): Add error location reporting.
#[derive(Debug, PartialEq, Eq, thiserror::Error, Clone)]
pub enum Error {
    #[error("expected `{expected}` but found `{found}`")]
    ExpectedSymbol {
        expected: TokenType,
        found: TokenType,
    },

    #[error("unexpected symbol `{symbol}`")]
    UnexpectedSymbol { symbol: TokenType },

    #[error("unexpected end of input")]
    UnexpectedEof,
}

type Result<T> = std::result::Result<T, Error>;

impl<I> Parser<I>
where
    I: Iterator<Item = Token>,
{
    /// Create a parser from any iterator of tokens
    pub fn new(tokens: I) -> Self {
        Self {
            tokens: tokens.peekable(),
        }
    }

    pub fn parse(mut self) -> Result<Expr> {
        let expr = self.expr()?;

        if let Some(token) = self.tokens.peek() {
            Err(Error::UnexpectedSymbol {
                symbol: token.typ.clone(),
            })
        } else {
            Ok(expr)
        }
    }

    fn expr(&mut self) -> Result<Expr> {
        let mut left = self.path()?;

        while let Some(token) = self.tokens.peek() {
            if token.typ == TokenType::Pipe {
                self.tokens.next(); // Consume explicit pipe
                let right = self.path()?;
                left = Expr::Pipe(Box::new(left), Box::new(right));
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn path(&mut self) -> Result<Expr> {
        let mut segments = Vec::new();

        // Check if it starts with identity (.)
        if let Some(token) = self.tokens.peek() {
            if token.typ == TokenType::Dot {
                self.tokens.next();
                // If only `.`, return Identity
                if !self.is_segment_start() {
                    return Ok(Expr::Identity);
                }
            }
        } else {
            return Err(Error::UnexpectedEof);
        }

        // Parse segments
        while self.is_segment_start() {
            segments.push(self.segment()?);
        }

        if segments.is_empty() {
            Ok(Expr::Identity)
        } else {
            Ok(Expr::Path(segments))
        }
    }

    fn is_segment_start(&mut self) -> bool {
        matches!(
            self.tokens.peek().map(|t| &t.typ),
            Some(TokenType::Field(_) | TokenType::LBracket)
        )
    }

    fn peek_optional(&mut self) -> bool {
        if let Some(token) = self.tokens.peek()
            && token.typ == TokenType::QuestionMark
        {
            self.tokens.next();
            return true;
        }
        false
    }

    fn segment(&mut self) -> Result<Segment> {
        match self.tokens.peek() {
            Some(token) => match &token.typ {
                TokenType::Field(s) => {
                    let result = s.clone();
                    self.tokens.next();
                    let optional = self.peek_optional();
                    Ok(Segment {
                        kind: SegmentKind::Field(result),
                        optional,
                    })
                }
                TokenType::LBracket => {
                    self.tokens.next(); // Consume `[`

                    match self.tokens.peek() {
                        Some(token) => match &token.typ {
                            TokenType::RBracket => {
                                self.tokens.next(); // Consume `]`
                                Ok(Segment {
                                    kind: SegmentKind::Each,
                                    optional: false,
                                })
                            }
                            TokenType::Integer(n) => {
                                let index = *n;
                                self.tokens.next();
                                self.consume(TokenType::RBracket)?;
                                let optional = self.peek_optional();
                                Ok(Segment {
                                    kind: SegmentKind::Index(index),
                                    optional,
                                })
                            }
                            unexpected => Err(Error::UnexpectedSymbol {
                                symbol: unexpected.clone(),
                            }),
                        },
                        None => Err(Error::UnexpectedEof),
                    }
                }
                unexpected => Err(Error::UnexpectedSymbol {
                    symbol: unexpected.clone(),
                }),
            },
            None => Err(Error::UnexpectedEof),
        }
    }

    /// Consume the current token if it matches the expected type, otherwise return an error.
    fn consume(&mut self, expected: TokenType) -> Result<Token> {
        let token = self.tokens.next().ok_or(Error::UnexpectedEof)?;
        if token.typ == expected {
            Ok(token)
        } else {
            Err(Error::ExpectedSymbol {
                expected,
                found: token.typ.clone(),
            })
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use super::super::lexer::Lexer;

    fn parse(input: &str) -> Result<Expr> {
        let tokens = Lexer::new(input).scan_tokens().unwrap();
        Parser::new(tokens.into_iter()).parse()
    }

    fn field(name: &str) -> Segment {
        Segment {
            kind: SegmentKind::Field(name.into()),
            optional: false,
        }
    }

    fn field_opt(name: &str) -> Segment {
        Segment {
            kind: SegmentKind::Field(name.into()),
            optional: true,
        }
    }

    fn index(n: u64) -> Segment {
        Segment {
            kind: SegmentKind::Index(n),
            optional: false,
        }
    }

    fn index_opt(n: u64) -> Segment {
        Segment {
            kind: SegmentKind::Index(n),
            optional: true,
        }
    }

    fn each() -> Segment {
        Segment {
            kind: SegmentKind::Each,
            optional: false,
        }
    }

    fn path(segments: Vec<Segment>) -> Expr {
        Expr::Path(segments)
    }

    fn pipe(left: Expr, right: Expr) -> Expr {
        Expr::Pipe(Box::new(left), Box::new(right))
    }

    #[test]
    fn basic() {
        assert_eq!(
            parse(".a.b.c"),
            Ok(path(vec![field("a"), field("b"), field("c")]))
        );
    }

    #[test]
    fn explicit_pipe() {
        assert_eq!(
            parse(".foo | .bar"),
            Ok(pipe(path(vec![field("foo")]), path(vec![field("bar")])))
        );
    }

    #[test]
    fn identity() {
        assert_eq!(parse("."), Ok(Expr::Identity));
    }

    #[test]
    fn identity_pipe() {
        assert_eq!(
            parse(". | .foo"),
            Ok(pipe(Expr::Identity, path(vec![field("foo")])))
        );
    }

    #[test]
    fn unexpected_eof() {
        assert_eq!(parse(".foo |"), Err(Error::UnexpectedEof));
    }

    #[test]
    fn empty_input() {
        assert_eq!(parse(""), Err(Error::UnexpectedEof));
    }

    #[test]
    fn array_index() {
        assert_eq!(parse(".[0]"), Ok(path(vec![index(0)])));
        assert_eq!(parse(".[42]"), Ok(path(vec![index(42)])));
    }

    #[test]
    fn array_index_with_pipe() {
        assert_eq!(
            parse(".foo | .[0]"),
            Ok(pipe(path(vec![field("foo")]), path(vec![index(0)])))
        );
    }

    #[test]
    fn array_index_implicit_pipe() {
        assert_eq!(parse(".foo[0]"), Ok(path(vec![field("foo"), index(0)])));
        assert_eq!(
            parse(".foo[0][1]"),
            Ok(path(vec![field("foo"), index(0), index(1)]))
        );
    }

    #[test]
    fn array_each() {
        assert_eq!(parse(".[]"), Ok(path(vec![each()])));
        assert_eq!(parse(".foo[]"), Ok(path(vec![field("foo"), each()])));
        assert_eq!(
            parse(".foo[] | .bar"),
            Ok(pipe(
                path(vec![field("foo"), each()]),
                path(vec![field("bar")])
            ))
        );
    }

    #[test]
    fn array_each_implicit_pipe() {
        assert_eq!(
            parse(".foo[].bar"),
            Ok(path(vec![field("foo"), each(), field("bar")]))
        );
        assert_eq!(
            parse(".foo[][0]"),
            Ok(path(vec![field("foo"), each(), index(0)]))
        );
    }

    #[test]
    fn array_index_errors() {
        assert_eq!(parse(".[0"), Err(Error::UnexpectedEof));
    }

    #[test]
    fn test_display_chain_vs_pipe() {
        let chain = parse(".location.x").unwrap();
        assert_eq!(chain.to_string(), ".location.x");

        let piped = parse(".foo | .bar").unwrap();
        assert_eq!(piped.to_string(), ".foo | .bar");

        let identity = parse(".").unwrap();
        assert_eq!(identity.to_string(), ".");

        let complex = parse(".a.b[] | .c[0]").unwrap();
        assert_eq!(complex.to_string(), ".a.b[] | .c[0]");
    }

    #[test]
    fn optional_field() {
        assert_eq!(parse(".foo?"), Ok(path(vec![field_opt("foo")])));
        assert_eq!(
            parse(".foo?.bar"),
            Ok(path(vec![field_opt("foo"), field("bar")]))
        );
    }

    #[test]
    fn optional_index() {
        assert_eq!(parse(".[0]?"), Ok(path(vec![index_opt(0)])));
    }

    #[test]
    fn optional_each_not_supported() {
        // `?` after `[]` should be a parse error (unexpected symbol)
        assert!(parse(".[]?").is_err());
    }

    #[test]
    fn test_display_optional() {
        let expr = parse(".foo?").unwrap();
        assert_eq!(expr.to_string(), ".foo?");

        let expr = parse(".foo?.bar").unwrap();
        assert_eq!(expr.to_string(), ".foo?.bar");

        // Note: leading `.` is consumed by the path parser, not stored in segments.
        let expr = parse(".[0]?").unwrap();
        assert_eq!(expr.to_string(), "[0]?");
    }
}
