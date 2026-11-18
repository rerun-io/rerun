//! `dyn_any_param`: functions should not take `&dyn Any`.
//!
//! `&Box<dyn Any>` itself implements `Any`, so it is easy to pass the wrong object.

use ra_ap_syntax::{AstNode as _, ast};

use super::{Rule, path_text};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "dyn_any_param";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: true,
    check_tests: true,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for param in file.syntax().descendants().filter_map(ast::Param::cast) {
        let Some(ast::Type::RefType(ref_type)) = param.ty() else {
            continue;
        };
        if ref_type.ty().is_some_and(|ty| is_dyn_any(&ty)) {
            diagnostics.push(Diagnostic::new(
                NAME,
                param.syntax().text_range().start(),
                "Functions should never take `&dyn std::any::Any` as argument, since `&Box<dyn std::any::Any>` \
                 itself implements `Any`, making it easy to accidentally pass the wrong object. \
                 Expect purpose-defined traits instead.",
            ));
        }
    }
}

fn is_dyn_any(ty: &ast::Type) -> bool {
    match ty {
        ast::Type::ParenType(paren) => paren.ty().is_some_and(|ty| is_dyn_any(&ty)),
        ast::Type::DynTraitType(dyn_trait) => dyn_trait.type_bound_list().is_some_and(|bounds| {
            bounds.bounds().any(|bound| {
                bound.ty().is_some_and(|ty| match ty {
                    ast::Type::PathType(path_type) => path_type.path().is_some_and(|path| {
                        matches!(
                            path_text(&path).as_str(),
                            "Any" | "std::any::Any" | "core::any::Any"
                        )
                    }),
                    _ => false,
                })
            })
        }),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    fn lint(src: &str) -> Vec<String> {
        lint_rule(super::NAME, src)
    }

    #[test]
    fn dyn_any() {
        assert_eq!(lint("fn f(x: &dyn Any) {}"), vec!["1: dyn_any_param"]);
        assert_eq!(
            lint("fn f(x: &mut dyn std::any::Any) {}"),
            vec!["1: dyn_any_param"]
        );
        assert_eq!(
            lint("fn f<'a>(x: &'a (dyn Any + Send)) {}"),
            vec!["1: dyn_any_param"]
        );
        assert_eq!(lint("fn f(x: Box<dyn Any>) {}"), Vec::<String>::new());
        assert_eq!(lint("fn f(x: &dyn Debug) {}"), Vec::<String>::new());
    }
}
