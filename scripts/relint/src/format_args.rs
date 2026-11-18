//! Understanding the arguments of `format!`-like macro calls.
//!
//! The parser does not expand macros, so a macro call is only a token tree.
//! This module splits that token tree into comma-separated arguments,
//! and parses the format string, so that rules can ask
//! "which argument does this `{…}` placeholder print?".

use ra_ap_syntax::{
    AstNode as _, AstToken as _, NodeOrToken, SyntaxElement, SyntaxKind, SyntaxToken, T, ast,
};

/// One comma-separated argument of a macro call, without surrounding trivia.
pub type MacroArg = Vec<SyntaxElement>;

/// Split the top level of a macro token tree on commas.
///
/// Nested delimited groups stay single elements.
pub fn macro_args(token_tree: &ast::TokenTree) -> Vec<MacroArg> {
    let mut args = Vec::new();
    let mut current = MacroArg::new();
    for element in token_tree.syntax().children_with_tokens() {
        match element.kind() {
            T!['('] | T![')'] | T!['['] | T![']'] | T!['{'] | T!['}']
                if is_outer_delimiter(token_tree, &element) => {}
            T![,] => args.push(std::mem::take(&mut current)),
            SyntaxKind::WHITESPACE | SyntaxKind::COMMENT => {}
            _ => current.push(element),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn is_outer_delimiter(token_tree: &ast::TokenTree, element: &SyntaxElement) -> bool {
    let node = token_tree.syntax();
    node.first_token().as_ref() == element.as_token()
        || node.last_token().as_ref() == element.as_token()
}

/// The argument as a single string literal, if it is one.
pub fn as_string_literal(arg: &[SyntaxElement]) -> Option<ast::String> {
    match arg {
        [NodeOrToken::Token(token)] => ast::String::cast(token.clone()),
        _ => None,
    }
}

/// The identifier the argument names, if it is a plain variable such as `err` or `&err`.
pub fn as_plain_ident(arg: &[SyntaxElement]) -> Option<SyntaxToken> {
    let mut tokens = Vec::new();
    for element in arg {
        let token = element.as_token()?;
        if token.kind() != T![&] {
            tokens.push(token);
        }
    }
    match tokens.as_slice() {
        [ident] if ident.kind() == SyntaxKind::IDENT => Some((*ident).clone()),
        _ => None,
    }
}

/// Which argument a placeholder refers to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgRef {
    /// `{}`: the next positional argument.
    Next,

    /// `{1}`: a positional argument by index.
    Index(usize),

    /// `{name}`: a named argument, or a variable captured from the surrounding scope.
    Name(String),
}

/// One `{…}` in a format string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placeholder {
    /// The argument it prints.
    pub arg: ArgRef,

    /// Uses `Debug` formatting (`{:?}`, `{x:#?}`, …).
    pub is_debug: bool,
}

/// Parse the placeholders of a format string (the unescaped literal value).
pub fn parse_placeholders(format: &str) -> Vec<Placeholder> {
    let mut placeholders = Vec::new();
    let mut chars = format.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        match c {
            '{' if chars.peek().is_some_and(|&(_, next)| next == '{') => {
                chars.next();
            }
            '{' => {
                let Some(len) = format[start + 1..].find('}') else {
                    break;
                };
                let inner = &format[start + 1..start + 1 + len];
                let (arg, spec) = inner.split_once(':').unwrap_or((inner, ""));
                let arg = arg.trim();
                let arg = if arg.is_empty() {
                    ArgRef::Next
                } else if let Ok(index) = arg.parse() {
                    ArgRef::Index(index)
                } else {
                    ArgRef::Name(arg.to_owned())
                };
                placeholders.push(Placeholder {
                    arg,
                    is_debug: spec.ends_with('?'),
                });
                while chars
                    .next_if(|&(index, _)| index <= start + len + 1)
                    .is_some()
                {}
            }
            _ => {}
        }
    }
    placeholders
}

/// The number of leading arguments of an `assert!`-like macro that come before its format string.
fn num_operands(macro_path: &str) -> usize {
    match macro_path.rsplit("::").next().unwrap_or(macro_path) {
        "assert" | "debug_assert" => 1,
        "assert_eq" | "assert_ne" | "debug_assert_eq" | "debug_assert_ne" => 2,
        _ => 0,
    }
}

/// What a placeholder prints.
pub enum Printed<'a> {
    /// A variable captured from the surrounding scope, e.g. `{err}`.
    Captured {
        /// The name of the variable.
        name: &'a str,
    },

    /// An explicit macro argument, e.g. the `err` in `format!("{}", err)`.
    Arg(&'a [SyntaxElement]),
}

/// A call to a `format!`-like macro: a format string, followed by its arguments.
pub struct FormatCall {
    /// The placeholders in the format string.
    pub placeholders: Vec<Placeholder>,

    /// Positional arguments after the format string.
    pub positional: Vec<MacroArg>,

    /// Named (`name = expr`) arguments after the format string.
    pub named: Vec<(String, MacroArg)>,
}

impl FormatCall {
    /// Find the format string of a call to the macro at `macro_path`:
    /// the first top-level string literal argument, after any leading operands.
    ///
    /// This covers `format!("…", …)`, `write!(f, "…", …)`, `assert_eq!(a, b, "…", …)`,
    /// and logging macros with leading structured fields, such as `warn!(?url, "…", …)`.
    pub fn parse(macro_path: &str, token_tree: &ast::TokenTree) -> Option<Self> {
        let args = macro_args(token_tree);
        let num_operands = num_operands(macro_path);
        let format_index = num_operands
            + args
                .iter()
                .skip(num_operands)
                .position(|arg| as_string_literal(arg).is_some())?;
        let literal = as_string_literal(&args[format_index])?;
        let placeholders = parse_placeholders(&literal.value().ok()?);

        let mut positional = Vec::new();
        let mut named = Vec::new();
        for arg in args.into_iter().skip(format_index + 1) {
            match arg.as_slice() {
                [NodeOrToken::Token(name), NodeOrToken::Token(eq), value @ ..]
                    if name.kind() == SyntaxKind::IDENT && eq.kind() == T![=] =>
                {
                    named.push((name.text().to_owned(), value.to_vec()));
                }
                _ => positional.push(arg),
            }
        }

        Some(Self {
            placeholders,
            positional,
            named,
        })
    }

    /// Pair every placeholder with what it prints.
    pub fn printed(&self) -> Vec<(&Placeholder, Printed<'_>)> {
        let mut next_positional = 0;
        let mut result = Vec::new();
        for placeholder in &self.placeholders {
            let printed = match &placeholder.arg {
                ArgRef::Next => {
                    next_positional += 1;
                    self.positional
                        .get(next_positional - 1)
                        .map(|arg| Printed::Arg(arg))
                }
                ArgRef::Index(index) => self.positional.get(*index).map(|arg| Printed::Arg(arg)),
                ArgRef::Name(name) => Some(
                    self.named
                        .iter()
                        .find(|(arg_name, _)| arg_name == name)
                        .map_or(Printed::Captured { name }, |(_, arg)| Printed::Arg(arg)),
                ),
            };
            if let Some(printed) = printed {
                result.push((placeholder, printed));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholder(arg: ArgRef, is_debug: bool) -> Placeholder {
        Placeholder { arg, is_debug }
    }

    #[test]
    fn placeholders() {
        assert_eq!(
            parse_placeholders("a {} {x:?} {1:#?} {{not}} {y:>8}"),
            vec![
                placeholder(ArgRef::Next, false),
                placeholder(ArgRef::Name("x".to_owned()), true),
                placeholder(ArgRef::Index(1), true),
                placeholder(ArgRef::Name("y".to_owned()), false),
            ]
        );
        assert_eq!(parse_placeholders("unclosed {"), vec![]);
        assert_eq!(parse_placeholders("}} {{}}"), vec![]);
    }
}
