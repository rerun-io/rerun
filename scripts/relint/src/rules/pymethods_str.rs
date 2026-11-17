//! `pymethods_str`: Python classes should have a useful `__str__` or `__repr__`.

use ra_ap_syntax::{
    AstNode as _,
    ast::{self, HasAttrs as _, HasName as _},
};

use super::{Rule, path_text, pyclass::pyclasses};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "pymethods_str";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: false,
    check_tests: true,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    let classes = pyclasses(file);

    // A class may spread its methods over several `#[pymethods]` blocks.
    let mut blocks: Vec<(String, ast::Attr, bool)> = Vec::new();
    for impl_ in file.syntax().descendants().filter_map(ast::Impl::cast) {
        let Some(attr) = impl_.attrs().find(|attr| {
            attr.path()
                .is_some_and(|path| path_text(&path) == "pymethods")
        }) else {
            continue;
        };
        let Some(ast::Type::PathType(self_ty)) = impl_.self_ty() else {
            continue;
        };
        let Some(rust_name) = self_ty
            .path()
            .and_then(|path| path.segment())
            .and_then(|segment| segment.name_ref())
            .map(|name| name.text().to_string())
        else {
            continue;
        };
        let has_str = impl_.assoc_item_list().is_some_and(|items| {
            items.assoc_items().any(|item| match item {
                ast::AssocItem::Fn(func) => func
                    .name()
                    .is_some_and(|name| matches!(name.text().as_str(), "__str__" | "__repr__")),
                _ => false,
            })
        });
        blocks.push((rust_name, attr, has_str));
    }

    let mut reported = Vec::new();
    for (rust_name, attr, _) in &blocks {
        if reported.contains(rust_name)
            || blocks
                .iter()
                .any(|(other, _, has_str)| other == rust_name && *has_str)
        {
            continue;
        }
        let class = classes.iter().find(|class| &class.rust_name == rust_name);
        let python_name = class.map_or(rust_name.as_str(), |class| class.python_name.as_str());
        // pyo3 gives enums a `__repr__` that prints the variant.
        if python_name.ends_with("Internal") || class.is_some_and(|class| class.is_enum) {
            continue;
        }
        reported.push(rust_name.clone());
        diagnostics.push(Diagnostic::new(
            NAME,
            attr.syntax().text_range().start(),
            format!(
                "`#[pymethods] impl {rust_name}` should include a `__str__` method (or `__repr__`, which serves as fallback)"
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    fn lint(src: &str) -> Vec<String> {
        lint_rule(super::NAME, src)
    }

    #[test]
    fn pymethods() {
        assert_eq!(
            lint("#[pymethods]\nimpl A {\n    fn __repr__(&self) {}\n}"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("#[pymethods]\nimpl<'py> A {\n    fn new() {}\n}"),
            vec!["1: pymethods_str"]
        );
        assert_eq!(
            lint("#[pymethods]\nimpl AInternal {\n    fn new() {}\n}"),
            Vec::<String>::new()
        );
        assert_eq!(lint("impl A {\n    fn new() {}\n}"), Vec::<String>::new());
    }

    #[test]
    fn python_names_enums_and_blocks() {
        assert_eq!(
            lint(
                "#[pyclass(name = \"AInternal\")]\nstruct PyA;\n#[pymethods]\nimpl PyA {\n    fn new() {}\n}"
            ),
            Vec::<String>::new(),
            "the Python name decides"
        );
        assert_eq!(
            lint(
                "#[pyclass(eq)]\nenum Kind { A }\n#[pymethods]\nimpl Kind {\n    fn f(&self) {}\n}"
            ),
            Vec::<String>::new(),
            "enums print their variant"
        );
        assert_eq!(
            lint(
                "#[pymethods]\nimpl A {\n    fn new() {}\n}\n#[pymethods]\nimpl A {\n    fn __repr__(&self) {}\n}"
            ),
            Vec::<String>::new(),
            "__repr__ in another block"
        );
    }
}
