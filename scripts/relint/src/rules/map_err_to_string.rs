//! `map_err_to_string`: don't throw away the error type with `.map_err(|err| err.to_string())`.
//!
//! Allowed in functions that return `Result<_, String>`, which `string_error` reports instead.

use ra_ap_syntax::{
    AstNode as _,
    ast::{self, HasArgList as _, HasName as _},
};

use super::{Rule, map_err_closure, path_text, string_error::is_string_result};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "map_err_to_string";

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
        let is_to_string = if let Some((name, closure)) = map_err_closure(&call) {
            closure
                .body()
                .is_some_and(|body| is_to_string_of(&unwrap_block(body), &name))
        } else {
            call.arg_list()
                .and_then(|args| args.args().next())
                .is_some_and(|arg| {
                    matches!(&arg, ast::Expr::PathExpr(path)
                        if path.path().is_some_and(|path| path_text(&path).ends_with("ToString::to_string")))
                })
                && call.name_ref().is_some_and(|name| name.text() == "map_err")
        };
        if is_to_string && !returns_string_result(&call) {
            diagnostics.push(Diagnostic::new(
                NAME,
                call.name_ref()
                    .map_or(call.syntax().text_range(), |name| name.syntax().text_range())
                    .start(),
                "`.map_err(|err| err.to_string())` throws away the error type: wrap the error instead",
            ));
        }
    }
}

/// `{ expr }` → `expr`.
fn unwrap_block(expr: ast::Expr) -> ast::Expr {
    if let ast::Expr::BlockExpr(block) = &expr
        && let Some(list) = block.stmt_list()
        && list.statements().next().is_none()
        && let Some(tail) = list.tail_expr()
    {
        unwrap_block(tail)
    } else {
        expr
    }
}

/// Is `expr` `name.to_string()`, `format!("{name}")`, or `format!("{}", name)`?
fn is_to_string_of(expr: &ast::Expr, name: &str) -> bool {
    if let ast::Expr::MacroExpr(expr) = expr
        && let Some(call) = expr.macro_call()
        && call
            .path()
            .is_some_and(|path| matches!(path_text(&path).as_str(), "format" | "std::format"))
        && let Some(tree) = call.token_tree()
    {
        let args: String = tree
            .syntax()
            .text()
            .to_string()
            .split_whitespace()
            .collect();
        return args == format!("(\"{{{name}}}\")")
            || args == format!("(\"{{}}\",{name})")
            || args == format!("(\"{{}}\",{name},)");
    }
    let ast::Expr::MethodCallExpr(call) = expr else {
        return false;
    };
    call.name_ref()
        .is_some_and(|method| method.text() == "to_string")
        && call
            .arg_list()
            .is_some_and(|args| args.args().next().is_none())
        && matches!(call.receiver(), Some(ast::Expr::PathExpr(receiver))
            if receiver.path().is_some_and(|path| path_text(&path) == name))
}

/// Does the function around `call` return `Result<_, String>`?
///
/// The closest closure with a return type counts too. Code in macro bodies is parsed inside a
/// placeholder `fn _()`, whose return type we don't know, so we give it the benefit of the doubt.
fn returns_string_result(call: &ast::MethodCallExpr) -> bool {
    for ancestor in call.syntax().ancestors() {
        let ret_type = if let Some(closure) = ast::ClosureExpr::cast(ancestor.clone()) {
            match closure.ret_type() {
                Some(ret_type) => ret_type,
                None => continue,
            }
        } else if let Some(function) = ast::Fn::cast(ancestor) {
            if function.name().is_none_or(|name| name.text() == "_") {
                return true;
            }
            match function.ret_type() {
                Some(ret_type) => ret_type,
                None => return false,
            }
        } else {
            continue;
        };
        return ret_type
            .syntax()
            .descendants()
            .filter_map(ast::PathSegment::cast)
            .any(|segment| is_string_result(&segment));
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    #[test]
    fn to_string() {
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() -> anyhow::Result<()> { x.map_err(|err| err.to_string())?; }"
            ),
            vec!["1: map_err_to_string"]
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() { x.map_err(|err| { err.to_string() }); }"
            ),
            vec!["1: map_err_to_string"],
            "block body, no return type"
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() { x.map_err(|err| format!(\"{err}\")); }"
            ),
            vec!["1: map_err_to_string"]
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() { x.map_err(|err| format!(\"failed: {err}\")); }"
            ),
            Vec::<String>::new(),
            "adds context"
        );
        assert_eq!(
            lint_rule(super::NAME, "fn f() { x.map_err(ToString::to_string); }"),
            vec!["1: map_err_to_string"]
        );
    }

    #[test]
    fn allowed() {
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() -> Result<(), String> { x.map_err(|err| err.to_string())?; }"
            ),
            Vec::<String>::new(),
            "function returns a String error"
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() -> anyhow::Result<()> {\n    let g = || -> Result<u8, String> { x.map_err(|err| err.to_string()) };\n}"
            ),
            Vec::<String>::new(),
            "closure returns a String error"
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                "fn f() { x.map_err(|err| Error::Parse(err.to_string())); }"
            ),
            Vec::<String>::new(),
            "wrapped"
        );
        assert_eq!(
            lint_rule(super::NAME, "fn f() { x.map(|v| v.to_string()); }"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn positional_format() {
        assert_eq!(
            lint_rule(
                super::NAME,
                r#"fn f() { x.map_err(|err| format!("{}", err)); }"#
            ),
            vec!["1: map_err_to_string"]
        );
        assert_eq!(
            lint_rule(
                super::NAME,
                r#"fn f() { x.map_err(|err| format!("{}: {}", path, err)); }"#
            ),
            Vec::<String>::new(),
            "adds context"
        );
    }
}
