//! `error_name`: errors should be called `err`, or have an `_err` suffix.

use ra_ap_syntax::{AstNode as _, ast::HasName as _};

use super::{Rule, error_bindings};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "error_name";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: true,
    check_tests: true,
};

/// Words that mean "error". A binding of an error whose name ends in one of these should be `err`.
///
/// Without types, we can't tell whether an `Err(x)` payload is really an error:
/// `binary_search` returns `Err(insertion_index)`, and `try_send` returns `Err(message)`.
/// So we only flag names that clearly mean "error".
const ERROR_WORDS: &[&str] = &[
    "e",
    "er",
    "error",
    "ex",
    "exc",
    "exception",
    "fail",
    "failure",
    "problem",
    "res",
    "status",
];

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for binding in error_bindings(file.syntax()) {
        let Some(name) = binding.name() else {
            continue;
        };
        let name = name.text();
        let last_word = name.rsplit('_').next().unwrap_or_default();
        if ERROR_WORDS.contains(&last_word) {
            diagnostics.push(Diagnostic::new(
                NAME,
                binding.syntax().text_range().start(),
                format!("Errors should be called `err` or have an `_err` suffix. Found: `{name}`"),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    fn lint(src: &str) -> Vec<String> {
        lint_rule(super::NAME, src)
    }

    #[test]
    fn patterns() {
        assert_eq!(
            lint("fn f() { if let Err(e) = x {} }"),
            vec!["1: error_name"]
        );
        assert_eq!(
            lint("fn f() { match x { Ok(_) => {}\n Err(error) => {} } }"),
            vec!["2: error_name"]
        );
        assert_eq!(
            lint("fn f() { if let Err(ref io_error) = x {} }"),
            vec!["1: error_name"]
        );
        assert_eq!(
            lint("fn f() { if let Err(err) = x {} }"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("fn f() { if let Err(parse_err) = x {} }"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("fn f() { if let Ok(res) = x {} }"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn closures() {
        assert_eq!(
            lint("fn f() { x.map_err(|e| e.to_string()); }"),
            vec!["1: error_name"]
        );
        assert_eq!(
            lint("fn f() { x\n    .unwrap_or_else(|error| {\n    }); }"),
            vec!["2: error_name"]
        );
        assert_eq!(
            lint("fn f() { x.inspect_err(|status| {}); }"),
            vec!["1: error_name"]
        );
        assert_eq!(
            lint("fn f() { x.map_err(|err| err.to_string()); }"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("fn f() { x.map(|e| e); }"),
            Vec::<String>::new(),
            "not an error closure"
        );
        assert_eq!(
            lint("fn f() { x.unwrap_or_else(|| 0); }"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn error_words() {
        assert_eq!(
            lint("fn f() { if let Err(io_error) = x {} }"),
            vec!["1: error_name"]
        );
        assert_eq!(lint("fn f() { x.map_err(|ex| 0); }"), vec!["1: error_name"]);
        assert_eq!(
            lint("fn f() { if let Err(parse_failure) = x {} }"),
            vec!["1: error_name"]
        );
        assert_eq!(
            lint(
                "fn f() { let i = v.binary_search(&x).unwrap_or_else(|insertion_index| insertion_index); }"
            ),
            Vec::<String>::new(),
            "not every Err payload is an error"
        );
        assert_eq!(
            lint("fn f() { if let Err(failures) = x {} }"),
            Vec::<String>::new(),
            "whole words"
        );
        assert_eq!(
            lint("fn f() { x.map(|signatures| 0); }"),
            Vec::<String>::new(),
            "not an error"
        );
    }
}
