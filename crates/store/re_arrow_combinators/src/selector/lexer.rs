//! Turns an input string into a list of [`Token`]s.
//!
//! The [`Lexer`] should roughly follow the structure from:
//! <https://github.com/jqlang/jq/blob/ea9e41f7587e2a515c9b7d28f3ab6ed00d30e5ce/src/lexer.l>

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenType {
    // Literals
    Field(String),
    Integer(u64), // TODO(grtlr): distinguish between float and integers.

    // Brackets
    LBracket,
    RBracket,

    // Operators
    Dot,
    Pipe,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error, Clone)]
pub enum Error {
    #[error("unexpected character `{ch}` at line {line}, column {column}")]
    UnexpectedChar {
        ch: char,
        line: usize,
        column: usize,
    },

    // TODO(grtlr): Add location information to other variants too (tricky because of line breaks).
    #[error("failed to parse `{lexeme}` as integer: {err}")]
    ParseIntError {
        err: std::num::ParseIntError,
        lexeme: String,
    },
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Field(s) => write!(f, ".{s}"),
            Self::Integer(n) => write!(f, "{n}"),
            Self::LBracket => write!(f, "["),
            Self::RBracket => write!(f, "]"),
            Self::Dot => write!(f, "."),
            Self::Pipe => write!(f, "|"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Token {
    pub typ: TokenType,
    pub line: usize,
}

pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    // TODO(grtlr): improve location support, for lexemes in particular
    line: usize,
    column: usize,
    lexeme_buffer: String,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            chars: source.chars().peekable(),
            line: 1,
            column: 1,
            lexeme_buffer: String::new(),
        }
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.next();
        if let Some(ch) = c {
            self.lexeme_buffer.push(ch);
            self.column += 1;
        }
        c
    }

    fn make_token(&self, typ: TokenType) -> Token {
        Token {
            typ,
            line: self.line,
        }
    }

    // TODO(grtlr): support quoted strings too
    fn make_field(&mut self) -> Token {
        while let Some(next) = self.chars.peek().copied()
            && (next.is_alphanumeric() || next == '-' || next == '_')
        {
            self.advance();
        }

        let text = std::mem::take(&mut self.lexeme_buffer);

        Token {
            typ: TokenType::Field(text),
            line: self.line,
        }
    }

    fn make_number(&mut self) -> Result<Token, Error> {
        while let Some(next) = self.chars.peek().copied()
            && next.is_ascii_digit()
        {
            self.advance();
        }

        let lexeme = std::mem::take(&mut self.lexeme_buffer);

        let number = lexeme
            .parse::<u64>()
            .map_err(|err| Error::ParseIntError { err, lexeme })?;

        Ok(Token {
            typ: TokenType::Integer(number),
            line: self.line,
        })
    }

    fn scan_token(&mut self) -> Result<Option<Token>, Error> {
        let c = self.advance().ok_or(Error::UnexpectedChar {
            ch: '\0',
            line: self.line,
            column: self.column,
        })?;

        match c {
            // Whitespace
            ' ' | '\r' | '\t' => Ok(None),
            '\n' => {
                self.line += 1;
                self.column = 1;
                Ok(None)
            }

            // Single-char tokens
            '|' => Ok(Some(self.make_token(TokenType::Pipe))),
            '[' => Ok(Some(self.make_token(TokenType::LBracket))),
            ']' => Ok(Some(self.make_token(TokenType::RBracket))),

            // Dot
            '.' => {
                if let Some(next) = self.chars.peek().copied()
                    && next.is_alphabetic()
                {
                    // Clear the '.' from lexeme_buffer before scanning the field
                    self.lexeme_buffer.clear();
                    Ok(Some(self.make_field()))
                } else {
                    Ok(Some(self.make_token(TokenType::Dot)))
                }
            }

            // Numbers
            '0'..='9' => self.make_number().map(Some),

            unexpected => Err(Error::UnexpectedChar {
                ch: unexpected,
                line: self.line,
                column: self.column - 1, // -1 because we already advanced
            }),
        }
    }

    pub fn scan_tokens(mut self) -> Result<Vec<Token>, Error> {
        let mut tokens = Vec::new();

        while self.chars.peek().is_some() {
            self.lexeme_buffer.clear();
            if let Some(token) = self.scan_token()? {
                tokens.push(token);
            } else {
                // Skip whitespace
            }
        }

        Ok(tokens)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn extract_inner(tokens: impl IntoIterator<Item = Token>) -> Vec<TokenType> {
        tokens.into_iter().map(|t| t.typ).collect()
    }

    #[test]
    fn basic() {
        assert_eq!(
            extract_inner(Lexer::new(".[]").scan_tokens().unwrap()),
            vec![TokenType::Dot, TokenType::LBracket, TokenType::RBracket,]
        );
        assert_eq!(
            extract_inner(Lexer::new(".[] | .").scan_tokens().unwrap()),
            vec![
                TokenType::Dot,
                TokenType::LBracket,
                TokenType::RBracket,
                TokenType::Pipe,
                TokenType::Dot,
            ]
        );
        assert_eq!(
            extract_inner(Lexer::new(".[] | .test_field").scan_tokens().unwrap()),
            vec![
                TokenType::Dot,
                TokenType::LBracket,
                TokenType::RBracket,
                TokenType::Pipe,
                TokenType::Field("test_field".into()),
            ]
        );
    }

    #[test]
    fn unexpected_char() {
        let result = Lexer::new(".foo @").scan_tokens();
        assert_eq!(
            result,
            Err(Error::UnexpectedChar {
                ch: '@',
                line: 1,
                column: 6
            })
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(
            extract_inner(Lexer::new("[0]").scan_tokens().unwrap()),
            vec![
                TokenType::LBracket,
                TokenType::Integer(0),
                TokenType::RBracket,
            ]
        );
        assert_eq!(
            extract_inner(Lexer::new("[42]").scan_tokens().unwrap()),
            vec![
                TokenType::LBracket,
                TokenType::Integer(42),
                TokenType::RBracket,
            ]
        );
        assert_eq!(
            extract_inner(Lexer::new(".foo[123]").scan_tokens().unwrap()),
            vec![
                TokenType::Field("foo".into()),
                TokenType::LBracket,
                TokenType::Integer(123),
                TokenType::RBracket,
            ]
        );
    }
}
