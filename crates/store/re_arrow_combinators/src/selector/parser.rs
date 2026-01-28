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
//! Term → FIELD
//!      | DOT
//!      | '[' INTEGER ']'
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Dot,
    Field(String),
    Index(u64),
    Each,
    Pipe(Box<Self>, Box<Self>),
}

// TODO(RR-3438): Add error location reporting.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
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
        let mut left = self.term()?;

        while let Some(token) = self.tokens.peek() {
            match &token.typ {
                TokenType::Pipe => {
                    self.tokens.next(); // Consume explicit pipe
                    let right = self.term()?;
                    left = Expr::Pipe(Box::new(left), Box::new(right));
                }
                TokenType::Field(_) | TokenType::Dot | TokenType::LBracket => {
                    // Implicit pipe (adjacent terms)
                    let right = self.term()?;
                    left = Expr::Pipe(Box::new(left), Box::new(right));
                }
                TokenType::Integer(_) | TokenType::RBracket => break,
            }
        }

        Ok(left)
    }

    fn term(&mut self) -> Result<Expr> {
        let token = self.tokens.peek().ok_or(Error::UnexpectedEof)?;
        match &token.typ {
            TokenType::Dot => {
                self.tokens.next();
                Ok(Expr::Dot)
            }
            TokenType::Field(s) => {
                let result = s.clone();
                self.tokens.next();
                Ok(Expr::Field(result))
            }
            TokenType::LBracket => {
                self.tokens.next(); // Consume `[`

                // Check if it's `[]` (Each) or `[n]` (Index)
                let token = self.tokens.peek().ok_or(Error::UnexpectedEof)?;
                match &token.typ {
                    TokenType::RBracket => {
                        self.tokens.next(); // Consume `]`
                        Ok(Expr::Each)
                    }
                    TokenType::Integer(n) => {
                        let index = *n;
                        self.tokens.next();
                        self.consume(TokenType::RBracket)?;
                        Ok(Expr::Index(index))
                    }
                    unexpected => Err(Error::UnexpectedSymbol {
                        symbol: unexpected.clone(),
                    }),
                }
            }
            unexpected => Err(Error::UnexpectedSymbol {
                symbol: unexpected.clone(),
            }),
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

    fn field(s: &str) -> Expr {
        Expr::Field(s.into())
    }

    fn pipe(left: Expr, right: Expr) -> Expr {
        Expr::Pipe(Box::new(left), Box::new(right))
    }

    #[test]
    fn basic() {
        assert_eq!(
            parse(".a .b .c"),
            Ok(pipe(pipe(field("a"), field("b")), field("c")))
        );
    }

    #[test]
    fn explicit_pipe() {
        assert_eq!(parse(".foo | .bar"), Ok(pipe(field("foo"), field("bar"))));
    }

    #[test]
    fn identity() {
        assert_eq!(parse("."), Ok(Expr::Dot));
    }

    #[test]
    fn identity_pipe() {
        assert_eq!(parse(". | .foo"), Ok(pipe(Expr::Dot, field("foo"))));
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
        assert_eq!(parse("[0]"), Ok(Expr::Index(0)));
        assert_eq!(parse("[42]"), Ok(Expr::Index(42)));
    }

    #[test]
    fn array_index_with_pipe() {
        assert_eq!(parse(".foo | [0]"), Ok(pipe(field("foo"), Expr::Index(0))));
    }

    #[test]
    fn array_index_implicit_pipe() {
        assert_eq!(parse(".foo[0]"), Ok(pipe(field("foo"), Expr::Index(0))));
        assert_eq!(
            parse(".foo[0][1]"),
            Ok(pipe(pipe(field("foo"), Expr::Index(0)), Expr::Index(1)))
        );
    }

    #[test]
    fn array_each() {
        assert_eq!(parse("[]"), Ok(Expr::Each));
        assert_eq!(parse(".foo[]"), Ok(pipe(field("foo"), Expr::Each)));
        assert_eq!(
            parse(".foo[] | .bar"),
            Ok(pipe(pipe(field("foo"), Expr::Each), field("bar")))
        );
    }

    #[test]
    fn array_each_implicit_pipe() {
        assert_eq!(
            parse(".foo[].bar"),
            Ok(pipe(pipe(field("foo"), Expr::Each), field("bar")))
        );
        assert_eq!(
            parse(".foo[][0]"),
            Ok(pipe(pipe(field("foo"), Expr::Each), Expr::Index(0)))
        );
    }

    #[test]
    fn array_index_errors() {
        // Missing closing bracket
        assert_eq!(parse(".a [0"), Err(Error::UnexpectedEof));
    }
}
