//! `pyclass_eq`, `pyclass_module`: requirements on `#[pyclass]` in our Python bindings.

use ra_ap_syntax::{
    AstNode as _, AstToken as _, SyntaxKind, SyntaxToken, T,
    ast::{self, HasName as _},
};

use super::{Rule, attr_token_tree, path_text};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const MODULE: &str = "rerun_bindings.rerun_bindings";

pub const EQ_RULE: Rule = Rule {
    name: "pyclass_eq",
    check_markdown: false,
    check_tests: true,
    check: |file, diagnostics| check(file, diagnostics, Requirement::Eq),
};

pub const MODULE_RULE: Rule = Rule {
    name: "pyclass_module",
    check_markdown: false,
    check_tests: true,
    check: |file, diagnostics| check(file, diagnostics, Requirement::Module),
};

#[derive(Clone, Copy)]
enum Requirement {
    Eq,
    Module,
}

/// A `#[pyclass]` struct or enum.
pub struct PyClass {
    /// The `#[pyclass(…)]` attribute.
    pub attr: ast::Attr,

    /// The tokens inside the attribute's parentheses, without trivia.
    pub tokens: Vec<SyntaxToken>,

    /// The name of the Rust type.
    pub rust_name: String,

    /// The name in Python: `name = "…"`, or else the Rust name.
    pub python_name: String,

    pub is_enum: bool,
}

/// All `#[pyclass]` types in a file.
pub fn pyclasses(file: &SourceFile) -> Vec<PyClass> {
    let mut classes = Vec::new();
    for attr in file.syntax().descendants().filter_map(ast::Attr::cast) {
        if attr.path().is_none_or(|path| path_text(&path) != "pyclass") {
            continue;
        }
        let tokens: Vec<SyntaxToken> = attr_token_tree(&attr)
            .map(|tt| {
                tt.syntax()
                    .children_with_tokens()
                    .filter_map(ra_ap_syntax::NodeOrToken::into_token)
                    .filter(|token| !token.kind().is_trivia())
                    .collect()
            })
            .unwrap_or_default();
        let Some(item) = attr.syntax().parent() else {
            continue;
        };
        let (rust_name, is_enum) = if let Some(s) = ast::Struct::cast(item.clone()) {
            (s.name(), false)
        } else if let Some(e) = ast::Enum::cast(item) {
            (e.name(), true)
        } else {
            continue;
        };
        let Some(rust_name) = rust_name.map(|name| name.text().to_string()) else {
            continue;
        };
        let python_name = key_value(&tokens, "name").unwrap_or_else(|| rust_name.clone());
        classes.push(PyClass {
            attr,
            tokens,
            rust_name,
            python_name,
            is_enum,
        });
    }
    classes
}

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>, requirement: Requirement) {
    for PyClass {
        attr,
        tokens,
        python_name,
        ..
    } in pyclasses(file)
    {
        if python_name.ends_with("Internal") {
            continue;
        }

        let offset = attr.syntax().text_range().start();
        match requirement {
            Requirement::Eq => {
                let has_eq = tokens
                    .iter()
                    .any(|token| token.kind() == SyntaxKind::IDENT && token.text() == "eq");
                if !has_eq {
                    diagnostics.push(Diagnostic::new(
                        EQ_RULE.name,
                        offset,
                        "`#[pyclass(…)]` should include the `eq` parameter, for Python equality support",
                    ));
                }
            }
            Requirement::Module => {
                if key_value(&tokens, "module").as_deref() != Some(MODULE) {
                    diagnostics.push(Diagnostic::new(
                        MODULE_RULE.name,
                        offset,
                        format!("`#[pyclass(…)]` should include `module = {MODULE:?}`"),
                    ));
                }
            }
        }
    }
}

/// The string value of `key = "value"` in the tokens of an attribute.
fn key_value(tokens: &[SyntaxToken], key: &str) -> Option<String> {
    tokens.windows(3).find_map(|window| match window {
        [k, eq, value]
            if k.kind() == SyntaxKind::IDENT && k.text() == key && eq.kind() == T![=] =>
        {
            ast::String::cast(value.clone())
                .and_then(|s| s.value().ok().map(std::borrow::Cow::into_owned))
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint;

    #[test]
    fn pyclass() {
        let good = "#[pyclass(eq, module = \"rerun_bindings.rerun_bindings\")]\nstruct A;";
        assert_eq!(lint(good), Vec::<String>::new());

        let multi_line = "#[pyclass(\n    // module = \"rerun_bindings.rerun_bindings\"\n    frozen,\n    eq\n)]\nstruct A;";
        assert_eq!(
            lint(multi_line),
            vec!["1: pyclass_module"],
            "comments do not count"
        );

        assert_eq!(
            lint("#[pyclass]\nstruct A;"),
            vec!["1: pyclass_eq", "1: pyclass_module"]
        );
        assert_eq!(
            lint("#[pyclass(frozen)]\nstruct AInternal;"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("#[pyclass(name = \"AInternal\")]\nstruct A;"),
            Vec::<String>::new()
        );
    }
}
