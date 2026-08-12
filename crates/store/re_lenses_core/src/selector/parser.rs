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
//! Expr        → Term ( '|' Term )*
//! Term        → Segment ( '?' | '!' )*  ( Segment ( '?' | '!' )* )*
//! Segment     → '.' FIELD
//!             | '[' INTEGER ']'
//!             | '[' ']'
//!             | '.'                          (identity)
//!             | 'map' '(' Expr ')'           (map)
//!             | 'pack' '(' PathExpr ( ',' PathExpr )* ')'  (pack into FixedSizeList)
//!             | IDENT ( '(' ArgList? ')' )?  (function)
//! ArgList     → Literal ( ';' Literal )*
//! Literal     → STRING_LITERAL
//!
//! PathExpr    → PathTerm ( '|' PathTerm )*
//! PathTerm    → PathSegment ( '?' | '!' )*  ( PathSegment ( '?' | '!' )* )*
//! PathSegment → '.' FIELD
//!             | '[' INTEGER ']'
//!             | '.'                          (identity)
//! ```
//!
//! `PathExpr` is the scalar-navigation subset accepted as a `pack` path: it deliberately
//! omits iteration (`[]`), `map`, functions, and nested `pack`, so those can never appear
//! as a path. The restriction is enforced directly by the `path_expr` production rather
//! than by parsing a full `Expr` and narrowing it afterwards.

// NOTE: Please keep the grammar above up-to-date.

use super::lexer::{Token, TokenType};

pub(super) struct Parser<I>
where
    I: Iterator<Item = Token>,
{
    tokens: std::iter::Peekable<I>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, re_byte_size::SizeBytes)]
pub enum Literal {
    String(String),
}

impl std::fmt::Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(v) => write!(f, "{v:?}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, re_byte_size::SizeBytes)]
pub enum Expr {
    Identity,
    Field(String),
    Index(u64),
    Each,
    Pipe {
        left: Box<Self>,
        right: Box<Self>,

        // TODO(RR-4178): Right now we still assume that `Selectors` have to
        // roundtrip in the UI, which is why we have to model if a pipe was
        // written out by the user in the AST. Long-term, we should avoid
        // coupling the Selector AST to the UI code.
        /// `true` when the pipe was inferred from adjacent segments (`.foo.bar`),
        /// `false` when the user wrote an explicit `|`.
        implicit: bool,
    },
    Try(Box<Self>),
    NonNull(Box<Self>),
    Function {
        name: String,

        /// This is `None` if the function was written as `my_func`, and
        /// is `Some([])` if it's written as `my_func()`. These should
        /// semantically be the same though.
        arguments: Option<Vec<Literal>>,
    },

    // TODO(grtlr): For now we define `map()` as an `Expr` in the tree. The
    // correct modeling would be to add the `map` function to the registry,
    // and defining it in terms of collect (`[ .[] | f]`).
    Map(Box<Self>),

    /// Packs the results of several 1:1 path expressions into a `FixedSizeList`.
    ///
    /// Written `pack(pathexpr, pathexpr, …)`, with at least one path. Each path is
    /// a [`PathExpr`] — a scalar navigation path that produces exactly one value
    /// per row — and all paths must share the same datatype. The resulting
    /// `FixedSizeList` has a size equal to the number of paths.
    ///
    /// The scalar-navigation restriction is enforced at parse time by the
    /// dedicated [`path_expr`](Parser::path_expr) production, so iteration (`[]`),
    /// `map`, functions, and nested `pack` can never appear as paths here.
    ///
    /// See the [module docs](super) for the nullability rules (the `!` gate and
    /// the entry-level AND semantics).
    Pack(Vec<PathExpr>),
}

/// A scalar navigation path: the restricted subset of [`Expr`] allowed as a
/// [`pack`](Expr::Pack) path.
///
/// A path produces **exactly one value per row**, so it can fill a fixed-size
/// slot. It mirrors the navigation-only fragment of [`Expr`]: identity (`.`),
/// field access (`.foo`), indexing (`[N]`), the `?`/`!` modifiers, and pipes of
/// those. The non-1:1 or dynamically-typed forms — iteration (`[]`), `map`,
/// functions, and nested `pack` — are simply not representable here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, re_byte_size::SizeBytes)]
pub enum PathExpr {
    /// See [`Expr::Identity`].
    Identity,

    /// See [`Expr::Field`].
    Field(String),

    /// See [`Expr::Index`].
    Index(u64),

    /// See [`Expr::Try`].
    Try(Box<Self>),

    /// See [`Expr::NonNull`].
    NonNull(Box<Self>),

    /// See [`Expr::Pipe`].
    Pipe {
        left: Box<Self>,
        right: Box<Self>,
        implicit: bool,
    },
}

/// Widen a path back into the full [`Expr`] grammar, for evaluation and display.
impl From<&PathExpr> for Expr {
    fn from(path: &PathExpr) -> Self {
        match path {
            PathExpr::Identity => Self::Identity,
            PathExpr::Field(name) => Self::Field(name.clone()),
            PathExpr::Index(index) => Self::Index(*index),
            PathExpr::Try(inner) => Self::Try(Box::new(Self::from(&**inner))),
            PathExpr::NonNull(inner) => Self::NonNull(Box::new(Self::from(&**inner))),
            PathExpr::Pipe {
                left,
                right,
                implicit,
            } => Self::Pipe {
                left: Box::new(Self::from(&**left)),
                right: Box::new(Self::from(&**right)),
                implicit: *implicit,
            },
        }
    }
}

impl std::fmt::Display for PathExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Expr::from(self))
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity => write!(f, "."),
            Self::Field(name) => write!(f, ".{name}"),
            Self::Index(n) => write!(f, "[{n}]"),
            Self::Each => write!(f, "[]"),
            Self::Pipe {
                left,
                right,
                implicit,
            } => {
                if *implicit {
                    write!(f, "{left}{right}")
                } else {
                    write!(f, "{left} | {right}")
                }
            }
            Self::Try(inner) => write!(f, "{inner}?"),
            Self::NonNull(inner) => write!(f, "{inner}!"),
            Self::Function { name, arguments } => {
                write!(f, "{name}")?;

                if let Some(arguments) = arguments {
                    write!(f, "(")?;
                    for (idx, literal) in arguments.iter().enumerate() {
                        if idx > 0 {
                            write!(f, "; ")?;
                        }
                        write!(f, "{literal}")?;
                    }
                    write!(f, ")")?;
                }

                Ok(())
            }
            Self::Map(body) => write!(f, "map({body})"),
            Self::Pack(paths) => {
                write!(f, "pack(")?;
                for (idx, path) in paths.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{path}")?;
                }
                write!(f, ")")
            }
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

    #[error(
        "`pack` paths must be scalar navigation paths — identity (`.`), field access (`.foo`), or indexing (`[N]`), optionally with `?`/`!` and pipes; iteration (`[]`), `map`, functions, and nested `pack` are not allowed (found `{symbol}`)"
    )]
    PackPathNotScalar { symbol: TokenType },
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
            if token.typ == TokenType::Pipe {
                self.tokens.next(); // Consume explicit pipe
                let right = self.term()?;
                left = Expr::Pipe {
                    left: Box::new(left),
                    right: Box::new(right),
                    implicit: false,
                };
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn term(&mut self) -> Result<Expr> {
        // Bare identifier: `map(expr)` or a function call
        if let Some(token) = self.tokens.peek()
            && let TokenType::Ident(name) = &token.typ
        {
            let name = name.clone();
            self.tokens.next();

            if name == "map" {
                return self.map_expr();
            }

            if name == "pack" {
                return self.pack_expr();
            }

            return self.function_args(name);
        }

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

        // Parse first segment
        let mut left = self.primary()?;
        left = self.postfix(left);

        // Parse remaining segments, joining with implicit pipes
        while self.is_segment_start() {
            let mut right = self.primary()?;
            right = self.postfix(right);
            left = Expr::Pipe {
                left: Box::new(left),
                right: Box::new(right),
                implicit: true,
            };
        }

        Ok(left)
    }

    /// Apply any postfix `?` or `!` operators.
    fn postfix(&mut self, mut expr: Expr) -> Expr {
        while let Some(token) = self.tokens.peek() {
            match token.typ {
                TokenType::QuestionMark => {
                    self.tokens.next();
                    expr = Expr::Try(Box::new(expr));
                }
                TokenType::ExclamationMark => {
                    self.tokens.next();
                    expr = Expr::NonNull(Box::new(expr));
                }
                _ => break,
            }
        }
        expr
    }

    /// Parse a `map(expr)` expression.
    /// The `map` identifier has already been consumed.
    fn map_expr(&mut self) -> Result<Expr> {
        self.consume(TokenType::LParen)?;
        let body = self.expr()?;
        self.consume(TokenType::RParen)?;
        Ok(Expr::Map(Box::new(body)))
    }

    /// Parse a `pack(path, path, …)` expression.
    /// The `pack` identifier has already been consumed.
    ///
    /// At least one path is required; an empty `pack()` is a parse error.
    fn pack_expr(&mut self) -> Result<Expr> {
        self.consume(TokenType::LParen)?;

        // Reject the empty `pack()` form: a `FixedSizeList` needs at least one element.
        if let Some(token) = self.tokens.peek()
            && token.typ == TokenType::RParen
        {
            return Err(Error::UnexpectedSymbol {
                symbol: TokenType::RParen,
            });
        }

        let mut paths = vec![self.path_expr()?];

        while let Some(token) = self.tokens.peek()
            && token.typ == TokenType::Comma
        {
            self.tokens.next();
            paths.push(self.path_expr()?);
        }

        self.consume(TokenType::RParen)?;

        Ok(Expr::Pack(paths))
    }

    /// Parse a [`PathExpr`]: the scalar-navigation subset accepted as a `pack` path.
    ///
    /// Mirrors [`expr`](Self::expr), but each piped section is a [`path_term`](Self::path_term)
    /// rather than a full [`term`](Self::term), so the navigation-only restriction is baked
    /// into the grammar instead of being checked after the fact.
    fn path_expr(&mut self) -> Result<PathExpr> {
        let mut left = self.path_term()?;

        while let Some(token) = self.tokens.peek() {
            if token.typ == TokenType::Pipe {
                self.tokens.next(); // Consume explicit pipe
                let right = self.path_term()?;
                left = PathExpr::Pipe {
                    left: Box::new(left),
                    right: Box::new(right),
                    implicit: false,
                };
            } else {
                break;
            }
        }

        Ok(left)
    }

    /// Parse a single path term: a chain of [`path_primary`](Self::path_primary) segments
    /// joined by implicit pipes, each with optional `?`/`!`.
    ///
    /// The counterpart of [`term`](Self::term), minus the `map`/`pack`/function dispatch:
    /// a path can only begin with `.` (identity, field, or index), never a bare identifier.
    fn path_term(&mut self) -> Result<PathExpr> {
        // A bare identifier here would be `map`, `pack`, or a function — none are paths.
        if let Some(token) = self.tokens.peek()
            && let TokenType::Ident(_) = &token.typ
        {
            return Err(Error::PackPathNotScalar {
                symbol: token.typ.clone(),
            });
        }

        // Leading `.`: bare identity, or the root of a `.[…]` index.
        if let Some(token) = self.tokens.peek() {
            if token.typ == TokenType::Dot {
                self.tokens.next();
                if !self.is_segment_start() {
                    return Ok(PathExpr::Identity);
                }
            }
        } else {
            return Err(Error::UnexpectedEof);
        }

        let mut left = self.path_primary()?;
        left = self.path_postfix(left);

        while self.is_segment_start() {
            let mut right = self.path_primary()?;
            right = self.path_postfix(right);
            left = PathExpr::Pipe {
                left: Box::new(left),
                right: Box::new(right),
                implicit: true,
            };
        }

        Ok(left)
    }

    /// Apply any postfix `?` or `!` operators to a path. Path counterpart of [`postfix`](Self::postfix).
    fn path_postfix(&mut self, mut path: PathExpr) -> PathExpr {
        while let Some(token) = self.tokens.peek() {
            match token.typ {
                TokenType::QuestionMark => {
                    self.tokens.next();
                    path = PathExpr::Try(Box::new(path));
                }
                TokenType::ExclamationMark => {
                    self.tokens.next();
                    path = PathExpr::NonNull(Box::new(path));
                }
                _ => break,
            }
        }
        path
    }

    /// Parse a single path segment: a field access or `[N]` index. Path counterpart of
    /// [`primary`](Self::primary), but `[]` (iteration) is rejected since a path must be 1:1.
    fn path_primary(&mut self) -> Result<PathExpr> {
        match self.tokens.peek() {
            Some(token) => match &token.typ {
                TokenType::Field(s) => {
                    let result = s.clone();
                    self.tokens.next();
                    Ok(PathExpr::Field(result))
                }
                TokenType::LBracket => {
                    self.tokens.next(); // Consume `[`

                    match self.tokens.peek() {
                        Some(token) => match &token.typ {
                            // `[]` is iteration — not a 1:1 path.
                            TokenType::RBracket => Err(Error::PackPathNotScalar {
                                symbol: TokenType::RBracket,
                            }),
                            TokenType::Integer(n) => {
                                let index = *n;
                                self.tokens.next();
                                self.consume(TokenType::RBracket)?;
                                Ok(PathExpr::Index(index))
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

    /// Parse function arguments: `(arg1; arg2; …)`.
    /// The `name` has already been consumed; parentheses are optional for no-arg calls.
    fn function_args(&mut self, name: String) -> Result<Expr> {
        // Allow bare function name without parentheses
        if self.tokens.peek().map(|t| &t.typ) != Some(&TokenType::LParen) {
            return Ok(Expr::Function {
                name,
                arguments: None,
            });
        }
        self.tokens.next(); // consume LParen

        let mut arguments = Vec::new();

        // Check for empty argument list
        if let Some(token) = self.tokens.peek()
            && token.typ == TokenType::RParen
        {
            self.tokens.next();
            return Ok(Expr::Function {
                name,
                arguments: Some(arguments),
            });
        }

        // Parse first argument
        arguments.push(self.literal()?);

        // Parse remaining semicolon-separated arguments
        while let Some(token) = self.tokens.peek()
            && token.typ == TokenType::Semicolon
        {
            self.tokens.next();
            arguments.push(self.literal()?);
        }

        self.consume(TokenType::RParen)?;

        Ok(Expr::Function {
            name,
            arguments: Some(arguments),
        })
    }

    fn literal(&mut self) -> Result<Literal> {
        match self.tokens.peek() {
            Some(token) => match &token.typ {
                TokenType::StringLiteral(s) => {
                    let value = s.clone();
                    self.tokens.next();
                    Ok(Literal::String(value))
                }
                unexpected => Err(Error::UnexpectedSymbol {
                    symbol: unexpected.clone(),
                }),
            },
            None => Err(Error::UnexpectedEof),
        }
    }

    fn is_segment_start(&mut self) -> bool {
        matches!(
            self.tokens.peek().map(|t| &t.typ),
            Some(TokenType::Field(_) | TokenType::LBracket)
        )
    }

    fn primary(&mut self) -> Result<Expr> {
        match self.tokens.peek() {
            Some(token) => match &token.typ {
                TokenType::Field(s) => {
                    let result = s.clone();
                    self.tokens.next();
                    Ok(Expr::Field(result))
                }
                TokenType::LBracket => {
                    self.tokens.next(); // Consume `[`

                    match self.tokens.peek() {
                        Some(token) => match &token.typ {
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

    fn field(name: &str) -> Expr {
        Expr::Field(name.into())
    }

    fn index(n: u64) -> Expr {
        Expr::Index(n)
    }

    fn each() -> Expr {
        Expr::Each
    }

    fn implicit_pipe(left: Expr, right: Expr) -> Expr {
        Expr::Pipe {
            left: Box::new(left),
            right: Box::new(right),
            implicit: true,
        }
    }

    fn try_expr(inner: Expr) -> Expr {
        Expr::Try(Box::new(inner))
    }

    fn non_null(inner: Expr) -> Expr {
        Expr::NonNull(Box::new(inner))
    }

    fn pipe(left: Expr, right: Expr) -> Expr {
        Expr::Pipe {
            left: Box::new(left),
            right: Box::new(right),
            implicit: false,
        }
    }

    #[test]
    fn basic() {
        assert_eq!(
            parse(".a.b.c"),
            Ok(implicit_pipe(
                implicit_pipe(field("a"), field("b")),
                field("c")
            ))
        );
    }

    #[test]
    fn explicit_pipe() {
        assert_eq!(parse(".foo | .bar"), Ok(pipe(field("foo"), field("bar"))));
    }

    #[test]
    fn identity() {
        assert_eq!(parse("."), Ok(Expr::Identity));
    }

    #[test]
    fn identity_pipe() {
        assert_eq!(parse(". | .foo"), Ok(pipe(Expr::Identity, field("foo"))));
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
        assert_eq!(parse(".[0]"), Ok(index(0)));
        assert_eq!(parse(".[42]"), Ok(index(42)));
    }

    #[test]
    fn array_index_with_pipe() {
        assert_eq!(parse(".foo | .[0]"), Ok(pipe(field("foo"), index(0))));
    }

    #[test]
    fn array_index_implicit_pipe() {
        assert_eq!(parse(".foo[0]"), Ok(implicit_pipe(field("foo"), index(0))));
        assert_eq!(
            parse(".foo[0][1]"),
            Ok(implicit_pipe(
                implicit_pipe(field("foo"), index(0)),
                index(1)
            ))
        );
    }

    #[test]
    fn array_each() {
        assert_eq!(parse(".[]"), Ok(each()));
        assert_eq!(parse(".foo[]"), Ok(implicit_pipe(field("foo"), each())));
        assert_eq!(
            parse(".foo[] | .bar"),
            Ok(pipe(implicit_pipe(field("foo"), each()), field("bar")))
        );
    }

    #[test]
    fn array_each_implicit_pipe() {
        assert_eq!(
            parse(".foo[].bar"),
            Ok(implicit_pipe(
                implicit_pipe(field("foo"), each()),
                field("bar")
            ))
        );
        assert_eq!(
            parse(".foo[][0]"),
            Ok(implicit_pipe(implicit_pipe(field("foo"), each()), index(0)))
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
        assert_eq!(parse(".foo?"), Ok(try_expr(field("foo"))));
        assert_eq!(
            parse(".foo?.bar"),
            Ok(implicit_pipe(try_expr(field("foo")), field("bar")))
        );
    }

    #[test]
    fn optional_index() {
        assert_eq!(parse(".[0]?"), Ok(try_expr(index(0))));
    }

    #[test]
    fn optional_each() {
        assert_eq!(parse(".[]?"), Ok(try_expr(each())));
        assert_eq!(
            parse(".[]?.foo"),
            Ok(implicit_pipe(try_expr(each()), field("foo")))
        );
        assert_eq!(
            parse(".foo[]?.bar"),
            Ok(implicit_pipe(
                implicit_pipe(field("foo"), try_expr(each())),
                field("bar")
            ))
        );
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

        let expr = parse(".[]?").unwrap();
        assert_eq!(expr.to_string(), "[]?");
    }

    #[test]
    fn non_null_field() {
        assert_eq!(parse(".foo!"), Ok(non_null(field("foo"))));
        assert_eq!(
            parse(".foo!.bar"),
            Ok(implicit_pipe(non_null(field("foo")), field("bar")))
        );
    }

    #[test]
    fn non_null_index() {
        assert_eq!(parse(".[0]!"), Ok(non_null(index(0))));
    }

    #[test]
    fn non_null_combined_with_optional() {
        assert_eq!(parse(".foo?!"), Ok(non_null(try_expr(field("foo")))));
        assert_eq!(parse(".foo!?"), Ok(try_expr(non_null(field("foo")))));
    }

    #[test]
    fn test_display_non_null() {
        let expr = parse(".foo!").unwrap();
        assert_eq!(expr.to_string(), ".foo!");

        let expr = parse(".foo!.bar").unwrap();
        assert_eq!(expr.to_string(), ".foo!.bar");

        let expr = parse(".[0]!").unwrap();
        assert_eq!(expr.to_string(), "[0]!");

        let expr = parse(".foo?!").unwrap();
        assert_eq!(expr.to_string(), ".foo?!");
    }

    fn func(name: &str, args: Option<Vec<Literal>>) -> Expr {
        Expr::Function {
            name: name.to_owned(),
            arguments: args,
        }
    }

    #[test]
    fn function_no_args() {
        assert_eq!(parse("my_func()"), Ok(func("my_func", Some(vec![]))));
        assert_eq!(parse("my_func"), Ok(func("my_func", None)));
    }

    #[test]
    fn function_one_arg() {
        assert_eq!(
            parse(r#"my_func("hello")"#),
            Ok(func("my_func", Some(vec![Literal::String("hello".into())])))
        );
    }

    #[test]
    fn function_multiple_args() {
        assert_eq!(
            parse(r#"my_func("foo"; "bar")"#),
            Ok(func(
                "my_func",
                Some(vec![
                    Literal::String("foo".into()),
                    Literal::String("bar".into())
                ])
            ))
        );
    }

    #[test]
    fn function_no_args_in_pipe() {
        assert_eq!(
            parse(".path | my_func"),
            Ok(pipe(field("path"), func("my_func", None)))
        );
    }

    #[test]
    fn function_in_pipe() {
        assert_eq!(
            parse(r#".path | my_func("arg")"#),
            Ok(pipe(
                field("path"),
                func("my_func", Some(vec![Literal::String("arg".into())]))
            ))
        );
    }

    #[test]
    fn function_display_roundtrip() {
        // `my_func` & `my_func()` are functionally the same, but we want
        // both to work for roundtrip.
        let expr = parse("my_func").unwrap();
        assert_eq!(expr.to_string(), "my_func");

        let expr = parse("my_func()").unwrap();
        assert_eq!(expr.to_string(), "my_func()");

        let expr = parse(r#"my_func("hello")"#).unwrap();
        assert_eq!(expr.to_string(), r#"my_func("hello")"#);

        let expr = parse(r#"my_func("foo"; "bar")"#).unwrap();
        assert_eq!(expr.to_string(), r#"my_func("foo"; "bar")"#);

        let expr = parse(r#".path | my_func("a"; "b")"#).unwrap();
        assert_eq!(expr.to_string(), r#".path | my_func("a"; "b")"#);
    }

    fn map_expr(body: Expr) -> Expr {
        Expr::Map(Box::new(body))
    }

    #[test]
    fn map_simple() {
        assert_eq!(parse("map(.foo)"), Ok(map_expr(field("foo"))));
    }

    #[test]
    fn map_with_pipe() {
        assert_eq!(
            parse("map(.foo | .bar)"),
            Ok(map_expr(pipe(field("foo"), field("bar"))))
        );
    }

    #[test]
    fn map_in_pipe() {
        assert_eq!(
            parse(".items | map(.name)"),
            Ok(pipe(field("items"), map_expr(field("name"))))
        );
    }

    #[test]
    fn map_display_roundtrip() {
        let expr = parse("map(.foo)").unwrap();
        assert_eq!(expr.to_string(), "map(.foo)");

        let expr = parse("map(.foo | .bar)").unwrap();
        assert_eq!(expr.to_string(), "map(.foo | .bar)");

        let expr = parse(".items | map(.name)").unwrap();
        assert_eq!(expr.to_string(), ".items | map(.name)");
    }

    fn pack(paths: Vec<PathExpr>) -> Expr {
        Expr::Pack(paths)
    }

    fn p_field(name: &str) -> PathExpr {
        PathExpr::Field(name.into())
    }

    fn p_non_null(inner: PathExpr) -> PathExpr {
        PathExpr::NonNull(Box::new(inner))
    }

    fn p_implicit_pipe(left: PathExpr, right: PathExpr) -> PathExpr {
        PathExpr::Pipe {
            left: Box::new(left),
            right: Box::new(right),
            implicit: true,
        }
    }

    #[test]
    fn pack_simple() {
        assert_eq!(
            parse("pack(.x, .y, .z)"),
            Ok(pack(vec![p_field("x"), p_field("y"), p_field("z")]))
        );
    }

    #[test]
    fn pack_single_path() {
        assert_eq!(parse("pack(.x)"), Ok(pack(vec![p_field("x")])));
    }

    #[test]
    fn pack_with_non_null_path() {
        assert_eq!(
            parse("pack(.x, .y!)"),
            Ok(pack(vec![p_field("x"), p_non_null(p_field("y"))]))
        );
    }

    #[test]
    fn pack_with_nested_path() {
        assert_eq!(
            parse("pack(.a.b, .c)"),
            Ok(pack(vec![
                p_implicit_pipe(p_field("a"), p_field("b")),
                p_field("c")
            ]))
        );
    }

    #[test]
    fn pack_empty_is_error() {
        assert_eq!(
            parse("pack()"),
            Err(Error::UnexpectedSymbol {
                symbol: TokenType::RParen
            })
        );
    }

    #[test]
    fn pack_non_scalar_path_is_parse_error() {
        // Each of these paths is non-1:1 or dynamically-typed, and is rejected by the
        // `path_expr` production at the offending token.
        for (query, symbol) in [
            ("pack(map(.x), .y)", TokenType::Ident("map".into())),
            ("pack(.x[], .y)", TokenType::RBracket),
            ("pack(.x, foo())", TokenType::Ident("foo".into())),
            ("pack(.x, pack(.y))", TokenType::Ident("pack".into())),
            // The offending token can follow a `?`/`!` segment or appear across a pipe.
            ("pack(.x[]!, .y)", TokenType::RBracket),
            ("pack(.x | foo, .y)", TokenType::Ident("foo".into())),
        ] {
            assert_eq!(
                parse(query),
                Err(Error::PackPathNotScalar { symbol }),
                "query: {query}"
            );
        }
    }

    #[test]
    fn pack_in_pipe() {
        assert_eq!(
            parse(".pose | pack(.x, .y)"),
            Ok(pipe(field("pose"), pack(vec![p_field("x"), p_field("y")])))
        );
    }

    #[test]
    fn pack_display_roundtrip() {
        let expr = parse("pack(.x, .y, .z)").unwrap();
        assert_eq!(expr.to_string(), "pack(.x, .y, .z)");

        let expr = parse("pack(.x, .y!)").unwrap();
        assert_eq!(expr.to_string(), "pack(.x, .y!)");

        let expr = parse(".pose | pack(.x, .y)").unwrap();
        assert_eq!(expr.to_string(), ".pose | pack(.x, .y)");

        let expr = parse("pack(.a.b, .c)").unwrap();
        assert_eq!(expr.to_string(), "pack(.a.b, .c)");

        let expr = parse("pack(.a | .b, .c)").unwrap();
        assert_eq!(expr.to_string(), "pack(.a | .b, .c)");
    }
}
