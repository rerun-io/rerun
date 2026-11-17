//! `inline_conversion`: trivial conversion methods should be `#[inline]`.
//!
//! These are called a lot, often across crates, where they would otherwise not be inlined.

use ra_ap_syntax::{
    AstNode as _,
    ast::{self, HasAttrs as _, HasName as _},
};

use super::{Rule, path_text};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "inline_conversion";

/// The methods of the conversion traits `AsRef`, `AsMut`, `Borrow`, `BorrowMut`, `Deref`, and `DerefMut`,
/// and inherent methods of the same names.
const METHODS: &[&str] = &[
    "as_mut",
    "as_ref",
    "borrow",
    "borrow_mut",
    "deref",
    "deref_mut",
];

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: true,
    check_tests: true,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for impl_ in file.syntax().descendants().filter_map(ast::Impl::cast) {
        let Some(items) = impl_.assoc_item_list() else {
            continue;
        };
        for item in items.assoc_items() {
            let ast::AssocItem::Fn(func) = item else {
                continue;
            };
            let Some(name) = func.name() else {
                continue;
            };
            let has_self = func
                .param_list()
                .is_some_and(|params| params.self_param().is_some());
            let is_inline = func
                .attrs()
                .any(|attr| attr.path().is_some_and(|path| path_text(&path) == "inline"));
            if METHODS.contains(&name.text().as_str())
                && has_self
                && func.body().is_some()
                && !is_inline
            {
                diagnostics.push(Diagnostic::new(
                    NAME,
                    name.syntax().text_range().start(),
                    format!("`fn {name}` should be marked `#[inline]`"),
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
    fn conversions() {
        assert_eq!(
            lint("impl Deref for A {\n    fn deref(&self) -> &B { &self.0 }\n}"),
            vec!["2: inline_conversion"]
        );
        assert_eq!(
            lint(
                "impl AsRef<B> for A {\n    #[inline]\n    /// Docs.\n    fn as_ref(&self) -> &B { &self.0 }\n}"
            ),
            Vec::<String>::new(),
            "a doc comment between the attribute and the fn is fine"
        );
        assert_eq!(
            lint(
                "impl Borrow<B> for A {\n    #[inline(always)]\n    fn borrow(&self) -> &B { &self.0 }\n}"
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("trait T {\n    fn deref(&self) -> &B;\n}"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("impl A {\n    fn deref(x: u32) {}\n}"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn mutable_conversions() {
        assert_eq!(
            lint("impl AsMut<B> for A {\n    fn as_mut(&mut self) -> &mut B { &mut self.0 }\n}"),
            vec!["2: inline_conversion"]
        );
        assert_eq!(
            lint(
                "impl BorrowMut<B> for A {\n    fn borrow_mut(&mut self) -> &mut B { &mut self.0 }\n}"
            ),
            vec!["2: inline_conversion"]
        );
    }
}
