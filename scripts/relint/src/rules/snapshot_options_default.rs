//! `snapshot_options_default`: snapshot tests should use our strict thresholds.
//!
//! `clippy.toml` forbids `SnapshotOptions::new`, but clippy cannot refer to trait methods such as `default`.

use ra_ap_syntax::{AstNode as _, ast};

use super::{Rule, path_text};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "snapshot_options_default";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: false,
    check_tests: true,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for path_expr in file.syntax().descendants().filter_map(ast::PathExpr::cast) {
        if path_expr
            .path()
            .is_some_and(|path| path_text(&path).ends_with("SnapshotOptions::default"))
        {
            diagnostics.push(Diagnostic::new(
                NAME,
                path_expr.syntax().text_range().start(),
                "Use `re_ui::testing::default_snapshot_options_for_ui/_3d` instead, so that snapshot tests get strict thresholds on CI",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    #[test]
    fn snapshot_options() {
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() { let o = egui_kittest::SnapshotOptions::default(); }"
            ),
            vec!["1: snapshot_options_default"]
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() { let o = default_snapshot_options_for_ui(); }"
            ),
            Vec::<String>::new()
        );
    }
}
