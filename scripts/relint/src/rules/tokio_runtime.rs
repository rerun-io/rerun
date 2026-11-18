//! `tokio_runtime`: create Tokio runtimes only at audited ownership boundaries.
//!
//! A process's `fn main` and test code own their runtimes, so they may create one.

use ra_ap_syntax::{
    AstNode as _,
    ast::{self, HasName as _},
};

use super::{Rule, path_text};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "tokio_runtime";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: false,
    check_tests: false,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for path_expr in file.syntax().descendants().filter_map(ast::PathExpr::cast) {
        let Some(path) = path_expr.path() else {
            continue;
        };
        let path = path_text(&path);
        let creates_runtime = path.ends_with("Builder::new_current_thread")
            || path.ends_with("Builder::new_multi_thread")
            || path.ends_with("runtime::Runtime::new");
        if creates_runtime && !is_in_main(path_expr.syntax()) {
            diagnostics.push(Diagnostic::new(
                NAME,
                path_expr.syntax().text_range().start(),
                "Create Tokio runtimes only at audited process or thread ownership boundaries. \
                 Library code should accept `re_async::AsyncRuntimeHandle`. \
                 If this runtime is required, add `// relint: allow(tokio_runtime): <who owns it>`.",
            ));
        }
    }
}

/// Is this inside the `fn main` of a binary (a top-level `fn main` in its file)?
fn is_in_main(node: &ra_ap_syntax::SyntaxNode) -> bool {
    node.ancestors().filter_map(ast::Fn::cast).any(|func| {
        func.name().is_some_and(|name| name.text() == "main")
            && func
                .syntax()
                .parent()
                .is_some_and(|parent| ast::SourceFile::can_cast(parent.kind()))
    })
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    fn lint(src: &str) -> Vec<String> {
        lint_rule(super::NAME, src)
    }

    #[test]
    fn runtimes() {
        assert_eq!(
            lint("fn f() { tokio::runtime::Builder::new_multi_thread(); }"),
            vec!["1: tokio_runtime"]
        );
        assert_eq!(
            lint("fn f() { Builder::new_current_thread(); }"),
            vec!["1: tokio_runtime"],
            "imported"
        );
        assert_eq!(
            lint("fn f() { tokio::runtime::Runtime::new(); }"),
            vec!["1: tokio_runtime"]
        );
        assert_eq!(
            lint("fn f() { tokio::runtime::Handle::current(); }"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn ownership_boundaries() {
        assert_eq!(
            lint("fn main() { tokio::runtime::Runtime::new(); }"),
            Vec::<String>::new(),
            "a binary's main"
        );
        assert_eq!(
            lint("fn main() { let rt = || { tokio::runtime::Runtime::new() }; }"),
            Vec::<String>::new(),
            "anywhere inside main"
        );
        assert_eq!(
            lint("mod m { fn main() { tokio::runtime::Runtime::new(); } }"),
            vec!["1: tokio_runtime"],
            "not a binary's main"
        );
        assert_eq!(
            lint("#[test]\nfn t() { tokio::runtime::Runtime::new(); }"),
            Vec::<String>::new(),
            "tests own their runtime"
        );
    }
}
