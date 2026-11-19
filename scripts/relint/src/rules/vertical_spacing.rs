//! `vertical_spacing`: separate declarations with a blank line, for readability.
//!
//! Applies to `fn`, `struct`, `enum`, `union`, `trait`, `type`, and `impl` items,
//! to items, fields, and enum variants that start with an attribute,
//! and to anything that starts with a `///` doc comment.
//! The first item in a block, consecutive bodiless `fn` declarations,
//! and consecutive `type` aliases need no blank line.

use ra_ap_syntax::{
    AstNode as _, AstToken as _, SyntaxKind, SyntaxNode, SyntaxToken, T, TextSize,
    ast::{self, HasAttrs as _},
};

use super::Rule;
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "vertical_spacing";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: false,
    check_tests: true,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for node in file.syntax().descendants() {
        if !needs_blank_line_before(&node) {
            continue;
        }
        let Some(mut first) = node.first_token() else {
            continue;
        };
        if ast::Comment::cast(first.clone()).is_some_and(|comment| !comment.is_doc()) {
            if starts_line(&first) {
                continue; // A plain comment right above an item introduces it.
            }
            // A trailing comment of the previous line, attached to this node by the parser.
            let Some(next) = first.next_token().and_then(|ws| ws.next_token()) else {
                continue;
            };
            first = next;
        }
        let Some(prev) = first.prev_token() else {
            continue;
        };
        if prev.kind() != SyntaxKind::WHITESPACE || is_separated(&prev, &node) {
            continue;
        }
        diagnostics.push(
            Diagnostic::new(
                NAME,
                first.text_range().start(),
                format!(
                    "For readability, add a blank line before `{}`",
                    first_line(&node)
                ),
            )
            .with_fix(line_start(&prev), "\n"),
        );
    }
}

fn needs_blank_line_before(node: &SyntaxNode) -> bool {
    match node.kind() {
        SyntaxKind::FN => ast::Fn::cast(node.clone()).is_some_and(|func| func.body().is_some()),
        SyntaxKind::STRUCT
        | SyntaxKind::ENUM
        | SyntaxKind::UNION
        | SyntaxKind::TRAIT
        | SyntaxKind::TYPE_ALIAS
        | SyntaxKind::IMPL => true,
        _ => {
            let Some(with_attrs) = ast::AnyHasAttrs::cast(node.clone()) else {
                return false;
            };
            let starts_with_doc_comment = node
                .first_token()
                .and_then(ast::Comment::cast)
                .is_some_and(|comment| comment.text().starts_with("///"));
            // Statements, match arms, and parameters with attributes need no blank line.
            // `use` items and `mod x;` declarations form groups, even with a `#[cfg(…)]` on some of them.
            let is_group_member = ast::Use::can_cast(node.kind())
                || ast::ExternCrate::can_cast(node.kind())
                || ast::Module::cast(node.clone())
                    .is_some_and(|module| module.item_list().is_none());
            let is_declaration = !is_group_member && ast::Item::can_cast(node.kind())
                || ast::AssocItem::can_cast(node.kind())
                || ast::ExternItem::can_cast(node.kind())
                || matches!(
                    node.kind(),
                    SyntaxKind::RECORD_FIELD | SyntaxKind::TUPLE_FIELD | SyntaxKind::VARIANT
                );
            let starts_with_attr = with_attrs.attrs().next().is_some_and(|attr| {
                attr.syntax().text_range().start() == node.text_range().start()
            });
            node.kind() != SyntaxKind::SOURCE_FILE
                && (starts_with_doc_comment || (is_declaration && starts_with_attr))
        }
    }
}

/// Is the whitespace `before` the node a blank line, or does the node not need one there?
fn is_separated(before: &SyntaxToken, node: &SyntaxNode) -> bool {
    if 2 <= before.text().matches('\n').count() || !before.text().contains('\n') {
        return true;
    }
    let Some(prev) = before.prev_token() else {
        return true;
    };
    match prev.kind() {
        // First in a block, or after an attribute or a `$(…)*` repetition that did not parse
        // as part of this item (e.g. `#[doc = $doc]` in a `macro_rules!` body).
        T!['{'] | T!['('] | T!['['] | T![']'] | T![*] => true,
        T![;] => node.kind() == SyntaxKind::TYPE_ALIAS,
        // A comment on a line of its own introduces what follows.
        SyntaxKind::COMMENT => starts_line(&prev),
        _ => false,
    }
}

/// Is `token` the first thing on its line?
fn starts_line(token: &SyntaxToken) -> bool {
    token
        .prev_token()
        .is_none_or(|prev| prev.kind() == SyntaxKind::WHITESPACE && prev.text().contains('\n'))
}

/// Where to insert the blank line: the start of the line the node is on.
fn line_start(whitespace_before: &SyntaxToken) -> TextSize {
    let last_newline = whitespace_before.text().rfind('\n').unwrap_or_default();
    whitespace_before.text_range().start()
        + TextSize::from(u32::try_from(last_newline + 1).unwrap_or_default())
}

fn first_line(node: &SyntaxNode) -> String {
    let text = node.text().to_string();
    text.lines().next().unwrap_or_default().trim().to_owned()
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    fn lint(src: &str) -> Vec<String> {
        lint_rule(super::NAME, src)
    }

    #[test]
    fn items() {
        assert_eq!(
            lint("fn foo() {}\nfn bar() {}\n"),
            vec!["2: vertical_spacing"]
        );
        assert_eq!(lint("fn foo() {}\n\nfn bar() {}\n"), Vec::<String>::new());
        assert_eq!(
            lint("async fn foo() {}\nasync fn bar() {}\n"),
            vec!["2: vertical_spacing"]
        );
        assert_eq!(
            lint("mod m {\n    fn foo() {}\n}\n"),
            Vec::<String>::new(),
            "first in block"
        );
        assert_eq!(
            lint("fn f() {\n    let x = 1;\n    fn helper() {}\n}\n"),
            vec!["3: vertical_spacing"]
        );
        assert_eq!(
            lint("struct A;\n#[serde(default)]\nstruct B;\n"),
            vec!["2: vertical_spacing"],
            "any attribute is part of the item"
        );
    }

    #[test]
    fn exceptions() {
        assert_eq!(
            lint("trait Foo {\n    fn bar();\n    fn baz();\n}\n"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint(
                "trait Foo {\n    type Response = Response<Body>;\n    type Error = hyper::Error;\n}\n"
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("type Callback =\n    fn(&mut egui::Ui, &dyn Array);\n"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("fn foo() {}\n// About bar:\nfn bar() {}\n"),
            Vec::<String>::new(),
            "a comment on its own line introduces the item"
        );
        assert_eq!(
            lint("fn foo() {} // trailing\nfn bar() {}\n"),
            vec!["2: vertical_spacing"]
        );
    }

    #[test]
    fn macro_followed_by_type() {
        assert_eq!(
            lint("slotmap::new_key_type! { pub struct Handle; }\ntype Map = SlotMap<Handle, V>;\n"),
            vec!["2: vertical_spacing"]
        );
    }

    #[test]
    fn docs_and_attributes() {
        assert_eq!(
            lint("struct S {\n    /// a\n    a: u32,\n\n    /// b\n    b: u32,\n}\n"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("struct S {\n    /// a\n    a: u32,\n    /// b\n    b: u32,\n}\n"),
            vec!["4: vertical_spacing"]
        );
        assert_eq!(
            lint("enum E {\n    Foo,\n    #[error(\"bar\")]\n    Bar,\n}\n"),
            vec!["3: vertical_spacing"]
        );
        assert_eq!(
            lint("struct S {\n    a: u32,\n    b: u32,\n}\n"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("enum E {\n    #[default]\n    A,\n    #[serde(rename = \"b\")]\n    B,\n}\n"),
            vec!["4: vertical_spacing"],
            "any attribute asks for a blank line"
        );
    }

    #[test]
    fn macro_rules_bodies() {
        let src = "macro_rules! m {\n    ($doc:literal, $name:ident) => {\n        #[doc = $doc]\n        pub fn $name() {}\n    };\n}\n";
        assert_eq!(
            lint(src),
            Vec::<String>::new(),
            "an attribute with a metavariable"
        );

        let src = "macro_rules! m {\n    ($(#[$meta:meta])*) => {\n        $(#[$meta])*\n        #[derive(Clone)]\n        pub struct S;\n    };\n}\n";
        assert_eq!(lint(src), Vec::<String>::new(), "after a repetition");
    }

    #[test]
    fn attributes_on_non_declarations() {
        assert_eq!(
            lint("fn f() {\n    let a = 1;\n    #[expect(unused)]\n    let b = 2;\n}\n"),
            Vec::<String>::new(),
            "statements"
        );
        assert_eq!(
            lint(
                "fn f() {\n    match x {\n        A => {}\n        #[cfg(test)]\n        B => {}\n    }\n}\n"
            ),
            Vec::<String>::new(),
            "match arms"
        );
        assert_eq!(
            lint("use a;\n#[cfg(test)]\nuse b;\n"),
            Vec::<String>::new(),
            "a group of uses"
        );
        assert_eq!(
            lint("mod a;\n#[cfg(test)]\nmod b;\n"),
            Vec::<String>::new(),
            "a group of mods"
        );
        assert_eq!(
            lint("fn f() {}\n#[cfg(test)]\nmod tests {}\n"),
            vec!["2: vertical_spacing"],
            "a mod with a body"
        );
    }
}
