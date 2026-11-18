//! `thiserror_positional`: use named fields for errors with more than one field.

use ra_ap_syntax::{AstNode as _, ast};

use super::{Rule, path_text};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "thiserror_positional";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: true,
    check_tests: true,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for attr in file.syntax().descendants().filter_map(ast::Attr::cast) {
        if attr.path().is_none_or(|path| path_text(&path) != "error") {
            continue;
        }
        let Some(item) = attr.syntax().parent() else {
            continue;
        };
        let field_list = ast::Variant::cast(item.clone())
            .and_then(|variant| variant.field_list())
            .or_else(|| ast::Struct::cast(item).and_then(|s| s.field_list()));
        let Some(ast::FieldList::TupleFieldList(fields)) = field_list else {
            continue;
        };
        if 2 <= fields.fields().count() {
            diagnostics.push(Diagnostic::new(
                NAME,
                attr.syntax().text_range().start(),
                "Use named fields for errors with more than one field, instead of unnamed tuple fields like {0}, {1}",
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
    fn positional() {
        assert_eq!(
            lint("enum E {\n    #[error(\"Failed {0}: {1}\")]\n    A(u32, u32),\n}"),
            vec!["2: thiserror_positional"]
        );
        assert_eq!(
            lint(
                "enum E {\n    #[error(\n        \"Failed to do {0}: {1:?}\"\n    )]\n    A(u32, u32),\n}"
            ),
            vec!["2: thiserror_positional"]
        );
        assert_eq!(
            lint("enum E {\n    #[error(\"Failed: {0}\")]\n    A(u32),\n}"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("enum E {\n    #[error(\"{path}: {err}\")]\n    A { path: P, err: E },\n}"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn fields_not_placeholders() {
        assert_eq!(
            lint("enum E {\n    #[error(\"Failed: {0}\")]\n    A(u32, String),\n}"),
            vec!["2: thiserror_positional"],
            "two unnamed fields, even if the message only uses one"
        );
        assert_eq!(
            lint("#[derive(thiserror::Error)]\n#[error(\"{0} and {1}\")]\nstruct E(u32, u32);"),
            vec!["2: thiserror_positional"],
            "a tuple struct"
        );
        assert_eq!(
            lint("enum E {\n    #[error(transparent)]\n    A(#[from] Io),\n}"),
            Vec::<String>::new()
        );
    }
}
