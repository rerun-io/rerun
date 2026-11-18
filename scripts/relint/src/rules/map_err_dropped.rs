//! `map_err_dropped`: `.map_err(|_err| …)` must use the error, by wrapping or logging it.
//!
//! Clippy's `map_err_ignore` only catches `|_|`.
//!
//! Errors that carry no information are exempt: those of `try_from`/`try_into` (e.g. an integer
//! out of range), of channel sends (the unsent value), of `Url::from_*_path` (`()`),
//! and of `timeout` (`Elapsed`).

use ra_ap_syntax::{AstNode as _, SyntaxKind, ast};

use super::{Rule, map_err_closure};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "map_err_dropped";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: true,
    check_tests: false,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for call in file
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
    {
        let Some((name, closure)) = map_err_closure(&call) else {
            continue;
        };
        let Some(body) = closure.body() else {
            continue;
        };
        let error_is_uninformative = call.receiver().is_some_and(|receiver| {
            callee_name(&receiver).is_some_and(|callee| returns_uninformative_error(&callee))
        });
        if error_is_uninformative {
            continue;
        }
        let is_used = body
            .syntax()
            .descendants_with_tokens()
            .filter_map(ra_ap_syntax::NodeOrToken::into_token)
            .any(|token| match token.kind() {
                SyntaxKind::IDENT => token.text() == name,
                SyntaxKind::STRING => {
                    let text = token.text();
                    text.contains(&format!("{{{name}}}")) || text.contains(&format!("{{{name}:"))
                }
                _ => false,
            });
        if !is_used {
            diagnostics.push(Diagnostic::new(
                NAME,
                closure.syntax().text_range().start(),
                format!("`map_err` drops the error `{name}`: wrap it in the new error, or log it"),
            ));
        }
    }
}

/// Does a function of this name return an error that is fine to drop, because it carries no information?
///
/// Without types, we go by the name of the function that produced the `Result`:
/// - `try_from`/`try_into`: e.g. `TryFromIntError`, which only says "out of range".
/// - Channel sends: the error holds the unsent value, not a reason.
/// - `Url::from_file_path`/`from_directory_path`: the error is `()`.
/// - `timeout`: the error is `Elapsed`.
fn returns_uninformative_error(callee: &str) -> bool {
    callee.starts_with("try_from")
        || callee.starts_with("try_into")
        || callee.ends_with("send")
        || callee.starts_with("send_")
        || callee.starts_with("from_directory_path")
        || callee.starts_with("from_file_path")
        || callee == "timeout"
}

/// The name of the function or method that produced `expr`, looking through `.await`.
fn callee_name(expr: &ast::Expr) -> Option<String> {
    match expr {
        ast::Expr::AwaitExpr(expr) => callee_name(&expr.expr()?),
        ast::Expr::MethodCallExpr(call) => Some(call.name_ref()?.text().to_string()),
        ast::Expr::CallExpr(call) => match call.expr()? {
            ast::Expr::PathExpr(path) => {
                Some(path.path()?.segment()?.name_ref()?.text().to_string())
            }
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    #[test]
    fn dropped() {
        assert_eq!(
            lint_rule(super::NAME, "fn f() { x.map_err(|_err| Error::Closed)?; }"),
            vec!["1: map_err_dropped"]
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() { x.map_err(|err| {\n    log(\"failed\");\n    Error::Closed\n})?; }"
            ),
            vec!["1: map_err_dropped"],
            "unused named parameter"
        );
    }

    #[test]
    fn used() {
        assert_eq!(
            lint_rule(super::NAME, "fn f() { x.map_err(|err| Error::Io(err))?; }"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() { x.map_err(|err| anyhow!(\"failed: {err}\"))?; }"
            ),
            Vec::<String>::new(),
            "used inside a macro"
        );
        assert_eq!(
            lint_rule(super::NAME, "fn f() { x.map_err(|_| Error::Closed)?; }"),
            Vec::<String>::new(),
            "`|_|` is clippy's `map_err_ignore`"
        );
        assert_eq!(
            lint_rule(super::NAME, "fn f() { x.map_err(Error::Io)?; }"),
            Vec::<String>::new(),
            "not a closure"
        );
    }

    #[test]
    fn errors_without_information() {
        for src in [
            "fn f() { u32::try_from(x).map_err(|_err| Error::TooLarge)?; }",
            "fn f() { x.try_into().map_err(|_err| Error::TooLarge)?; }",
            "fn f() { tx.blocking_send(cmd).map_err(|_err| Error::Closed)?; }",
            "fn f() { tokio::time::timeout(d, fut).await.map_err(|_elapsed| Error::Timeout)?; }",
            "fn f() { Url::from_directory_path(p).map_err(|_err| Error::NotAbsolute)?; }",
        ] {
            assert_eq!(lint_rule(super::NAME, src), Vec::<String>::new(), "{src}");
        }
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() { s.parse().map_err(|_err| Error::Invalid)?; }"
            ),
            vec!["1: map_err_dropped"],
            "parse errors say what is wrong"
        );
    }
}
