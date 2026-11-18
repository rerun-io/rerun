//! `debug_formatted_error`: format errors with `Display`, not `Debug`.
//!
//! `Debug` output of errors is noisy and unhelpful to users.

use ra_ap_syntax::{AstNode as _, SyntaxKind, T, ast};

use super::{Rule, is_error_ident, macro_invocations};
use crate::{
    diagnostic::Diagnostic,
    format_args::{FormatCall, Printed, as_plain_ident},
    source::SourceFile,
};

const NAME: &str = "debug_formatted_error";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: true,
    // In tests, the `Debug` form (with the variant name) is what a failing test should show.
    check_tests: false,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for invocation in macro_invocations(file) {
        let Some(format_call) = FormatCall::parse(&invocation.path, &invocation.token_tree) else {
            continue;
        };
        for (placeholder, printed) in format_call.printed() {
            let name = match printed {
                Printed::Captured { name } => Some(name.to_owned()),
                Printed::Arg(arg) => as_plain_ident(arg).map(|ident| ident.text().to_owned()),
            };
            if placeholder.is_debug && name.is_some_and(|name| is_error_ident(&name)) {
                diagnostics.push(Diagnostic::new(
                    NAME,
                    invocation.offset,
                    "Format errors with `re_error::format` or using `Display` - not `Debug` formatting!",
                ));
            }
        }
    }

    // `tracing` field syntax: `?err` records the field with `Debug`.
    // Nested macros are covered, since we look at all tokens in the tree.
    for call in file.syntax().descendants().filter_map(ast::MacroCall::cast) {
        let Some(token_tree) = call.token_tree() else {
            continue;
        };
        let tokens: Vec<_> = token_tree
            .syntax()
            .descendants_with_tokens()
            .filter_map(ra_ap_syntax::NodeOrToken::into_token)
            .filter(|token| !token.kind().is_trivia())
            .collect();
        for window in tokens.windows(3) {
            let [before, question, ident] = window else {
                continue;
            };
            if question.kind() == T![?]
                && matches!(before.kind(), T![,] | T!['('] | T!['{'] | T![=])
                && ident.kind() == SyntaxKind::IDENT
                && is_error_ident(ident.text())
            {
                diagnostics.push(Diagnostic::new(
                    NAME,
                    question.text_range().start(),
                    format!(
                        "Use `%{0}` (Display) instead of `?{0}` (Debug) in tracing macros",
                        ident.text()
                    ),
                ));
            }
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
    fn inline() {
        assert_eq!(
            lint(r#"fn f() { format!("{err:?}"); }"#),
            vec!["1: debug_formatted_error"]
        );
        assert_eq!(
            lint(r#"fn f() { format!("{parse_err:#?}"); }"#),
            vec!["1: debug_formatted_error"]
        );
        assert_eq!(
            lint(r#"fn f() { format!("{err}"); }"#),
            Vec::<String>::new()
        );
        assert_eq!(
            lint(r#"fn f() { format!("{value:?}"); }"#),
            Vec::<String>::new()
        );
    }

    #[test]
    fn positional_across_lines() {
        let src = "fn f() {\n    re_log::warn!(\n        \"Failed: {:?}\",\n        err\n    );\n}";
        assert_eq!(lint(src), vec!["2: debug_formatted_error"]);
        assert_eq!(
            lint(r#"fn f() { println!("{} {:?}", err, value); }"#),
            Vec::<String>::new()
        );
        assert_eq!(
            lint(r#"fn f() { println!("{1:?}", value, &err); }"#),
            vec!["1: debug_formatted_error"]
        );
    }

    #[test]
    fn named_arg() {
        assert_eq!(
            lint(r#"fn f() { println!("{e:?}", e = err); }"#),
            vec!["1: debug_formatted_error"]
        );
        assert_eq!(
            lint(r#"fn f() { println!("{err:?}", err = value); }"#),
            Vec::<String>::new()
        );
    }

    #[test]
    fn tracing_fields() {
        assert_eq!(
            lint(r#"fn f() { tracing::warn!(?err, "failed"); }"#),
            vec!["1: debug_formatted_error"]
        );
        assert_eq!(
            lint(r#"fn f() { tracing::warn!(error = ?err, "failed"); }"#),
            vec!["1: debug_formatted_error"]
        );
        assert_eq!(
            lint(r#"fn f() { tracing::warn!(%err, ?value, "failed"); }"#),
            Vec::<String>::new()
        );
        assert_eq!(lint(r"fn f() { m!(foo()?, err); }"), Vec::<String>::new());
    }

    #[test]
    fn nested_macros() {
        assert_eq!(
            lint("fn f() {\n    assert_eq!(\n        format!(\"{err:?}\"),\n        x\n    );\n}"),
            vec!["3: debug_formatted_error"]
        );
        assert_eq!(
            lint(r#"fn f() { vec![std::format!("{:?}", err)]; }"#),
            vec!["1: debug_formatted_error"]
        );
    }

    #[test]
    fn assert_operands() {
        assert_eq!(
            lint(r#"fn f() { assert_eq!(a, "lit", "{err:?}"); }"#),
            vec!["1: debug_formatted_error"]
        );
        assert_eq!(
            lint(r#"fn f() { assert!(s == "x", "{err:?}"); }"#),
            vec!["1: debug_formatted_error"]
        );
        assert_eq!(
            lint(r#"fn f() { assert_eq!("{err:?}", s); }"#),
            Vec::<String>::new(),
            "an operand, not a format string"
        );
    }

    #[test]
    fn outside_macros() {
        assert_eq!(
            lint(r#"fn f() { let s = "{err:?}"; }"#),
            Vec::<String>::new()
        );
    }

    #[test]
    fn whole_words() {
        assert_eq!(
            lint(r#"fn f() { println!("{stderr:?}"); }"#),
            Vec::<String>::new()
        );
        assert_eq!(
            lint(r#"fn f() { m!(::std::format!("{err:?}")); }"#),
            vec!["1: debug_formatted_error"]
        );
    }

    #[test]
    fn macro_arguments() {
        assert_eq!(
            lint(
                "fn f() {\n    assert!(x.map(|v| {\n        format!(\"{err:?}\")\n    }).is_ok());\n}"
            ),
            vec!["3: debug_formatted_error"],
            "found once, from either parse"
        );
    }
}
