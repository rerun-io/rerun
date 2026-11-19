//! `string_error`: use a `thiserror` enum as the error type, not a `String`.
//!
//! A `String` error cannot be matched on, and loses the source of the error.

use ra_ap_syntax::{
    AstNode as _,
    ast::{self, HasGenericArgs as _, HasGenericParams as _, HasName as _},
};

use super::{Rule, path_text};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "string_error";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: false,
    check_tests: false,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for segment in file
        .syntax()
        .descendants()
        .filter_map(ast::PathSegment::cast)
    {
        if is_string_result(&segment) {
            diagnostics.push(Diagnostic::new(
                NAME,
                segment.syntax().text_range().start(),
                "Use a `thiserror` enum as the error type instead of `String`",
            ));
        }
    }

    // `impl FromStr for Foo { type Err = String; … }`, and `type Error = String` for `TryFrom`.
    for alias in file
        .syntax()
        .descendants()
        .filter_map(ast::TypeAlias::cast)
        .filter(|alias| {
            alias
                .syntax()
                .parent()
                .is_some_and(|parent| ast::AssocItemList::can_cast(parent.kind()))
        })
    {
        let is_error_type = alias
            .name()
            .is_some_and(|name| matches!(name.text().as_str(), "Err" | "Error"));
        if is_error_type
            && let Some(ty) = alias.ty()
            && is_string(&ty)
        {
            diagnostics.push(Diagnostic::new(
                NAME,
                ty.syntax().text_range().start(),
                "Use a `thiserror` enum as the error type instead of `String`",
            ));
        }
    }

    // `type Result<T, E = String> = std::result::Result<T, E>;`
    for alias in file.syntax().descendants().filter_map(ast::TypeAlias::cast) {
        if alias.name().is_none_or(|name| name.text() != "Result") {
            continue;
        }
        let defaults = alias
            .generic_param_list()
            .into_iter()
            .flat_map(|params| params.type_or_const_params())
            .filter_map(|param| match param {
                ast::TypeOrConstParam::Type(param) => param.default_type(),
                ast::TypeOrConstParam::Const(_) => None,
            });
        for default in defaults {
            if is_string(&default) {
                diagnostics.push(Diagnostic::new(
                    NAME,
                    default.syntax().text_range().start(),
                    "Use a `thiserror` enum as the default error type instead of `String`",
                ));
            }
        }
    }
}

fn is_string(ty: &ast::Type) -> bool {
    matches!(ty, ast::Type::PathType(ty)
    if ty.path().is_some_and(|path| matches!(
        path_text(&path).as_str(),
        "String" | "std::string::String" | "alloc::string::String"
    )))
}

/// Is this path segment `Result<T, String>`?
pub fn is_string_result(segment: &ast::PathSegment) -> bool {
    if segment
        .name_ref()
        .is_none_or(|name| name.text() != "Result")
    {
        return false;
    }
    let Some(generic_args) = segment.generic_arg_list() else {
        return false;
    };
    let args: Vec<ast::GenericArg> = generic_args.generic_args().collect();
    matches!(args.as_slice(), [_, ast::GenericArg::TypeArg(error)]
        if error.ty().is_some_and(|error| is_string(&error)))
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    #[test]
    fn string_error() {
        assert_eq!(
            lint_rule(super::NAME, "fn f() -> Result<(), String> {}"),
            vec!["1: string_error"]
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() -> std::result::Result<\n    u8,\n    std::string::String,\n> {}"
            ),
            vec!["1: string_error"],
            "multi-line, full paths"
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "struct S { f: Box<dyn Fn() -> Result<u8, String>> }"
            ),
            vec!["1: string_error"],
            "nested in another type"
        );
        assert_eq!(
            lint_rule(super::NAME, "fn f() -> Result<String, Error> {}"),
            Vec::<String>::new(),
            "`String` as the value"
        );
        assert_eq!(
            lint_rule(super::NAME, "fn f() -> Result<(), MyString> {}"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "type Result<T, E = String> = std::result::Result<T, E>;"
            ),
            vec!["1: string_error"],
            "default error type of an alias"
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "type Result<T, E = Error> = std::result::Result<T, E>;"
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "impl FromStr for Foo {\n    type Err = String;\n}\nimpl TryFrom<u8> for Foo {\n    type Error = std::string::String;\n}"
            ),
            vec!["2: string_error", "5: string_error"],
            "associated error types"
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "impl Foo for Bar {\n    type Output = String;\n}\ntype Error = String;"
            ),
            Vec::<String>::new(),
            "other associated types, and free aliases"
        );
    }

    #[test]
    fn tests_are_allowed() {
        assert_eq!(
            lint_rule(
                super::NAME,
                "#[cfg(test)]\nmod tests {\n    fn f() -> Result<(), String> {}\n}"
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "#[tokio::test]\nasync fn f() -> Result<(), String> {}"
            ),
            Vec::<String>::new()
        );
        for src in [
            "#[tokio::test(flavor = \"multi_thread\")]\nasync fn f() -> Result<(), String> {}",
            "#[rstest]\nfn f() -> Result<(), String> {}",
            "#[cfg(all(test, not(target_arch = \"wasm32\")))]\nmod t {\n    fn f() -> Result<(), String> {}\n}",
            "#![cfg(test)]\nfn f() -> Result<(), String> {}",
        ] {
            assert_eq!(lint_rule(super::NAME, src), Vec::<String>::new(), "{src}");
        }
        assert_eq!(
            lint_rule(
                super::NAME,
                "#[cfg(any(test, feature = \"testing\"))]\nfn f() -> Result<(), String> {}"
            ),
            vec!["2: string_error"],
            "`any(test, …)` is also built outside tests"
        );
        for path in ["src/tests.rs", "src/table/lmdb_tests.rs", "tests/common.rs"] {
            let file = crate::source::SourceFile::parse(
                path.to_owned(),
                "fn f() -> Result<(), String> {}".to_owned(),
            );
            let rules: Vec<&crate::rules::Rule> = vec![&super::RULE];
            assert!(crate::rules::check_file(&file, &rules).is_empty(), "{path}");
        }
    }
}
