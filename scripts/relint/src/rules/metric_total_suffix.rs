//! `metric_total_suffix`: counter names must not end in `_total`.
//!
//! The Prometheus exporter appends `_total` to every monotonic counter,
//! so a baked-in `_total` is exported doubled (`foo_total_total`).

use ra_ap_syntax::{
    AstNode as _, AstToken as _,
    ast::{self, HasArgList as _},
};

use super::{Rule, path_text};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "metric_total_suffix";

/// Builders of monotonic counters. Not `*_up_down_counter`, which gets no suffix.
const COUNTER_BUILDERS: &[&str] = &[
    "f64_counter",
    "f64_observable_counter",
    "u64_counter",
    "u64_observable_counter",
];

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: false,
    check_tests: true,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for node in file.syntax().descendants() {
        let (name, args) = if let Some(call) = ast::MethodCallExpr::cast(node.clone()) {
            (
                call.name_ref().map(|name| name.text().to_string()),
                call.arg_list(),
            )
        } else if let Some(call) = ast::CallExpr::cast(node) {
            let name = match call.expr() {
                Some(ast::Expr::PathExpr(path)) => path
                    .path()
                    .and_then(|path| path_text(&path).rsplit("::").next().map(str::to_owned)),
                _ => None,
            };
            (name, call.arg_list())
        } else {
            continue;
        };
        if !name.is_some_and(|name| COUNTER_BUILDERS.contains(&name.as_str())) {
            continue;
        }
        let Some(ast::Expr::Literal(literal)) = args.and_then(|args| args.args().next()) else {
            continue;
        };
        let Some(metric) = ast::String::cast(literal.token())
            .and_then(|s| s.value().ok().map(std::borrow::Cow::into_owned))
        else {
            continue;
        };
        if let Some(fixed) = metric.strip_suffix("_total") {
            diagnostics.push(Diagnostic::new(
                NAME,
                literal.syntax().text_range().start(),
                format!(
                    "Counter {metric:?} must not end in `_total`, because the exporter appends it. Rename it to {fixed:?}."
                ),
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
    fn counters() {
        assert_eq!(
            lint(r#"fn f() { m.u64_counter("foo_total"); }"#),
            vec!["1: metric_total_suffix"]
        );
        assert_eq!(
            lint(r#"fn f() { m.f64_observable_counter("foo_total"); }"#),
            vec!["1: metric_total_suffix"]
        );
        assert_eq!(
            lint(
                "fn f() {\n    m\n        .u64_counter(\n            \"foo_total\",\n        );\n}"
            ),
            vec!["4: metric_total_suffix"]
        );
        assert_eq!(
            lint(r#"fn f() { Meter::u64_counter(&m, "foo_total"); }"#),
            Vec::<String>::new(),
            "the name is not the first argument"
        );
        assert_eq!(
            lint(r#"fn f() { m.u64_counter("foo"); }"#),
            Vec::<String>::new()
        );
        assert_eq!(
            lint(r#"fn f() { m.i64_up_down_counter("foo_total"); }"#),
            Vec::<String>::new()
        );
        assert_eq!(
            lint(r#"fn f() { m.u64_gauge("foo_total"); }"#),
            Vec::<String>::new()
        );
    }
}
