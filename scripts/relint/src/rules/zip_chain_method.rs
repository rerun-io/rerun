//! `zip_chain_method`: prefer the free functions `std::iter::zip` and `std::iter::chain`.
//!
//! The free functions read symmetrically, and are easier to extend to more inputs.

use ra_ap_syntax::{
    AstNode as _,
    ast::{self, HasArgList as _},
};

use super::Rule;
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "zip_chain_method";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: false,
    check_tests: true,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for call in file
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
    {
        let Some(name) = call.name_ref() else {
            continue;
        };
        let num_args = call.arg_list().map_or(0, |args| args.args().count());
        let message = match name.text().as_str() {
            "zip" if num_args == 1 => {
                "Prefer `std::iter::zip(a, b)` (iterators), `itertools::izip!(a, b, …)` (3+ iterators), \
                 or `Option::zip(a, b)` (options) over `a.zip(b)`"
            }
            "chain" if num_args == 1 => {
                "Prefer `std::iter::chain(a, b)` or `itertools::chain!(a, b, …)` over `a.chain(b)`"
            }
            _ => continue,
        };
        if looks_like_stream(&call) {
            continue;
        }
        diagnostics.push(Diagnostic::new(
            NAME,
            name.syntax().text_range().start(),
            message,
        ));
    }
}

/// Methods that only async streams have, not iterators.
const STREAM_ONLY_METHODS: &[&str] = &[
    "boxed",
    "buffer_unordered",
    "buffered",
    "instrument_polls",
    "then",
    "try_buffer_unordered",
    "try_buffered",
];

/// Does the call look like `Stream::chain` or `Stream::zip`, which have no free-function form?
///
/// Without types, we look for `stream` in a path or name, or a stream-only method
/// in the receiver's call chain.
fn looks_like_stream(call: &ast::MethodCallExpr) -> bool {
    let mentions_stream = |node: &ra_ap_syntax::SyntaxNode| {
        node.descendants_with_tokens()
            .filter_map(ra_ap_syntax::NodeOrToken::into_token)
            .any(|token| {
                token.kind() == ra_ap_syntax::SyntaxKind::IDENT
                    && token.text().to_lowercase().contains("stream")
            })
    };
    let receiver_has_stream_method = call.receiver().is_some_and(|receiver| {
        std::iter::chain(
            receiver
                .syntax()
                .descendants()
                .filter_map(ast::MethodCallExpr::cast),
            ast::MethodCallExpr::cast(receiver.syntax().clone()),
        )
        .any(|inner| {
            inner
                .name_ref()
                .is_some_and(|name| STREAM_ONLY_METHODS.contains(&name.text().as_str()))
        })
    });
    call.receiver()
        .is_some_and(|receiver| mentions_stream(receiver.syntax()))
        || call
            .arg_list()
            .is_some_and(|args| mentions_stream(args.syntax()))
        || receiver_has_stream_method
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    fn lint(src: &str) -> Vec<String> {
        lint_rule(super::NAME, src)
    }

    #[test]
    fn methods() {
        assert_eq!(
            lint("fn f() { a.iter()\n    .zip(b); }"),
            vec!["2: zip_chain_method"]
        );
        assert_eq!(lint("fn f() { a.chain(b); }"), vec!["1: zip_chain_method"]);
        assert_eq!(
            lint("fn f() { std::iter::zip(a, b); }"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("fn f() { err.chain(); }"),
            Vec::<String>::new(),
            "e.g. `anyhow::Error::chain`"
        );
        assert_eq!(
            lint("// a.zip(b)\nfn f() { let s = \"a.zip(b)\"; }"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn macro_arguments() {
        assert_eq!(
            lint("fn f() { assert!(a.iter().zip(b).all(|x| x)); }"),
            vec!["1: zip_chain_method"]
        );
        assert_eq!(
            lint("fn f() { vec![a.chain(b)]; }"),
            vec!["1: zip_chain_method"]
        );
        assert_eq!(
            lint("m! { x.zip(y); }"),
            vec!["1: zip_chain_method"],
            "at the start of the file"
        );
        assert_eq!(
            lint("macro_rules! m {\n    ($x:expr) => {\n        $x.zip(y)\n    };\n}"),
            vec!["3: zip_chain_method"],
            "macro_rules! bodies"
        );
    }

    #[test]
    fn streams() {
        assert_eq!(
            lint("fn f() { futures::stream::iter(a).chain(b); }"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("fn f() { first.chain(rest_stream); }"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("fn f() { a.then(|x| async { x }).chain(b); }"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint("fn f() { a.iter().map(f).chain(b); }"),
            vec!["1: zip_chain_method"]
        );
    }
}
