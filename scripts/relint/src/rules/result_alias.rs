//! `anyhow_result`, `tonic_result`: prefer the crate's own `Result` alias.

use ra_ap_syntax::{
    AstNode as _,
    ast::{self, HasGenericArgs as _},
};

use super::{Rule, path_text};
use crate::{diagnostic::Diagnostic, source::SourceFile};

pub const ANYHOW_RULE: Rule = Rule {
    name: "anyhow_result",
    check_markdown: true,
    check_tests: true,
    check: |file, diagnostics| {
        check(
            file,
            diagnostics,
            "anyhow_result",
            "anyhow::Error",
            "anyhow::Result",
        );
    },
};

pub const TONIC_RULE: Rule = Rule {
    name: "tonic_result",
    check_markdown: false,
    check_tests: true,
    check: |file, diagnostics| {
        check(
            file,
            diagnostics,
            "tonic_result",
            "tonic::Status",
            "tonic::Result",
        );
    },
};

fn check(
    file: &SourceFile,
    diagnostics: &mut Vec<Diagnostic>,
    rule: &'static str,
    error_type: &str,
    alias: &str,
) {
    // Any `Result<…>` segment: in a type, or in an expression such as `Result::<T, E>::Ok(…)`.
    for segment in file
        .syntax()
        .descendants()
        .filter_map(ast::PathSegment::cast)
    {
        if segment
            .name_ref()
            .is_none_or(|name| name.text() != "Result")
        {
            continue;
        }
        let Some(generic_args) = segment.generic_arg_list() else {
            continue;
        };
        let args: Vec<ast::GenericArg> = generic_args.generic_args().collect();
        if let [_, ast::GenericArg::TypeArg(error)] = args.as_slice()
            && let Some(ast::Type::PathType(error)) = error.ty()
            && error
                .path()
                .is_some_and(|path| path_text(&path) == error_type)
        {
            diagnostics.push(Diagnostic::new(
                rule,
                segment.syntax().text_range().start(),
                format!("Prefer using `{alias}<T>` over `Result<T, {error_type}>`"),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    #[test]
    fn anyhow() {
        assert_eq!(
            lint_rule("anyhow_result", "fn f() -> Result<(), anyhow::Error> {}"),
            vec!["1: anyhow_result"]
        );
        assert_eq!(
            lint_rule(
                "anyhow_result",
                "fn f() -> std::result::Result<\n    Vec<u8>,\n    anyhow::Error,\n> {}"
            ),
            vec!["1: anyhow_result"],
            "multi-line"
        );
        assert_eq!(
            lint_rule(
                "anyhow_result",
                "fn f() { Result::<(), anyhow::Error>::Ok(()) }"
            ),
            vec!["1: anyhow_result"],
            "in an expression"
        );
        assert_eq!(
            lint_rule("anyhow_result", "fn f() -> anyhow::Result<()> {}"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint_rule("anyhow_result", "fn f() -> Result<(), my::Error> {}"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn tonic() {
        assert_eq!(
            lint_rule(
                "tonic_result",
                "fn f() -> Result<\n    Response<T>,\n    tonic::Status,\n> {}"
            ),
            vec!["1: tonic_result"]
        );
        assert_eq!(
            lint_rule("tonic_result", "fn f() -> tonic::Result<()> {}"),
            Vec::<String>::new()
        );
    }
}
