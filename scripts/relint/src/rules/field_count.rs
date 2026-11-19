//! Limits on the number of fields in structs and enum variants.
//!
//! `positional_fields`: tuple structs and tuple variants have at most one field.
//! With more, the reader has to remember which position means what.
//!
//! `variant_fields`: enum variants have at most [`MAX_VARIANT_FIELDS`] named fields.
//! Larger variants should hold a struct instead, which can be named, documented, and passed around.
//! Variants with an `#[error(…)]` attribute are exempt: their fields are the context of the message.
//!
//! `PhantomData` fields carry no data, so neither rule counts them.

use ra_ap_syntax::{
    AstNode as _,
    ast::{self, HasName as _},
};

use super::{Rule, path_text};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const POSITIONAL_NAME: &str = "positional_fields";
const VARIANT_NAME: &str = "variant_fields";

const MAX_POSITIONAL_FIELDS: usize = 1;

/// A variant with more fields is a struct in disguise,
/// and four fields still fit in a pattern on one line.
const MAX_VARIANT_FIELDS: usize = 4;

pub const POSITIONAL_RULE: Rule = Rule {
    name: POSITIONAL_NAME,
    check: check_positional,
    check_markdown: true,
    check_tests: true,
};

pub const VARIANT_RULE: Rule = Rule {
    name: VARIANT_NAME,
    check: check_variant,
    check_markdown: true,
    check_tests: true,
};

/// The name and fields of every struct and enum variant in the file.
fn items_with_fields(file: &SourceFile) -> impl Iterator<Item = (ast::Name, ast::FieldList)> {
    file.syntax().descendants().filter_map(|node| {
        if let Some(variant) = ast::Variant::cast(node.clone()) {
            Some((variant.name()?, variant.field_list()?))
        } else {
            let strukt = ast::Struct::cast(node)?;
            Some((strukt.name()?, strukt.field_list()?))
        }
    })
}

/// `PhantomData` carries no data, so it does not count as a field.
fn is_phantom(ty: Option<ast::Type>) -> bool {
    let Some(ast::Type::PathType(ty)) = ty else {
        return false;
    };
    ty.path()
        .and_then(|path| path.segment())
        .and_then(|segment| segment.name_ref())
        .is_some_and(|name| name.text() == "PhantomData")
}

fn check_positional(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for (name, fields) in items_with_fields(file) {
        let ast::FieldList::TupleFieldList(fields) = fields else {
            continue;
        };
        let count = fields
            .fields()
            .filter(|field| !is_phantom(field.ty()))
            .count();
        if MAX_POSITIONAL_FIELDS < count {
            diagnostics.push(Diagnostic::new(
                POSITIONAL_NAME,
                name.syntax().text_range().start(),
                format!("`{name}` has {count} unnamed fields. Use named fields instead"),
            ));
        }
    }
}

fn check_variant(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for variant in file.syntax().descendants().filter_map(ast::Variant::cast) {
        let (Some(name), Some(ast::FieldList::RecordFieldList(fields))) =
            (variant.name(), variant.field_list())
        else {
            continue;
        };
        let is_error = ast::HasAttrs::attrs(&variant)
            .any(|attr| attr.path().is_some_and(|path| path_text(&path) == "error"));
        if is_error {
            continue;
        }
        let count = fields
            .fields()
            .filter(|field| !is_phantom(field.ty()))
            .count();
        if MAX_VARIANT_FIELDS < count {
            diagnostics.push(Diagnostic::new(
                VARIANT_NAME,
                name.syntax().text_range().start(),
                format!(
                    "Variant `{name}` has {count} fields (max {MAX_VARIANT_FIELDS}). \
                     Move them into a struct, and hold that in the variant"
                ),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    fn positional(src: &str) -> Vec<String> {
        lint_rule(super::POSITIONAL_NAME, src)
    }

    fn variant(src: &str) -> Vec<String> {
        lint_rule(super::VARIANT_NAME, src)
    }

    #[test]
    fn tuple_structs() {
        assert_eq!(
            positional("struct A(u32, String);"),
            vec!["1: positional_fields"]
        );
        assert_eq!(positional("struct A(pub u32);"), Vec::<String>::new());
        assert_eq!(positional("struct A();"), Vec::<String>::new());
        assert_eq!(
            positional("struct A<T>(u32, std::marker::PhantomData<T>);"),
            Vec::<String>::new()
        );
        assert_eq!(
            positional("struct A { a: u32, b: u32 }"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn tuple_variants() {
        assert_eq!(
            positional("enum E {\n    A,\n    B(u32),\n    C(u32, String),\n}"),
            vec!["4: positional_fields"]
        );
        assert_eq!(
            positional("enum E<T> {\n    A(u32, PhantomData<T>),\n}"),
            Vec::<String>::new(),
            "`PhantomData` carries no data"
        );
        assert_eq!(
            positional("enum E<T> {\n    A(u32, u32, PhantomData<T>),\n}"),
            vec!["2: positional_fields"]
        );
    }

    #[test]
    fn restricted_visibility() {
        assert_eq!(
            positional("pub struct A(pub(crate) u32, pub(crate) String);"),
            vec!["1: positional_fields"]
        );
        assert_eq!(
            positional("pub struct A(pub(crate) u32);"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn errors() {
        assert_eq!(
            positional("enum E {\n    #[error(\"{0}: {1}\")]\n    A(u32, u32),\n}"),
            vec!["3: positional_fields"],
            "only named fields are exempt for errors"
        );
        assert_eq!(
            positional("enum E {\n    #[error(transparent)]\n    A(#[from] Io),\n}"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn named_variants() {
        assert_eq!(
            variant("enum E {\n    A { a: u8, b: u8, c: u8, d: u8 },\n}"),
            Vec::<String>::new()
        );
        assert_eq!(
            variant("enum E {\n    A { a: u8, b: u8, c: u8, d: u8, e: u8 },\n}"),
            vec!["2: variant_fields"]
        );
        assert_eq!(
            variant("struct S { a: u8, b: u8, c: u8, d: u8, e: u8 }"),
            Vec::<String>::new(),
            "structs are not limited"
        );
        assert_eq!(
            variant("enum E<T> {\n    A { a: u8, b: u8, c: u8, d: u8, e: PhantomData<T> },\n}"),
            Vec::<String>::new(),
            "`PhantomData` carries no data"
        );
        assert_eq!(
            variant(
                "enum E {\n    #[error(\"{a}{b}{c}{d}{e}\")]\n    A { a: u8, b: u8, c: u8, d: u8, e: u8 },\n}"
            ),
            Vec::<String>::new(),
            "error variants carry the context of their message"
        );
    }
}
