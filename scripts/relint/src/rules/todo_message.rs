//! `todo_message`: `todo!()` and `unimplemented!()` should explain what is missing.

use super::{Rule, macro_args, macro_invocations};
use crate::{diagnostic::Diagnostic, source::SourceFile};

const NAME: &str = "todo_message";

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: true,
    check_tests: true,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for invocation in macro_invocations(file) {
        let name = invocation.path.rsplit("::").next().unwrap_or_default();
        let is_std = invocation.path == name
            || invocation.path.starts_with("std::")
            || invocation.path.starts_with("core::");
        let is_placeholder = is_std && matches!(name, "todo" | "unimplemented");
        if is_placeholder && macro_args(&invocation.token_tree).is_empty() {
            diagnostics.push(Diagnostic::new(
                NAME,
                invocation.offset,
                format!("`{name}!()` should explain what is missing: `{name}!(\"$details\")`"),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    #[test]
    fn todo() {
        assert_eq!(
            lint_rule(super::NAME, "fn f() { todo!() }"),
            vec!["1: todo_message"]
        );
        assert_eq!(
            lint_rule(super::NAME, "fn f() { todo!(\"support tensors\") }"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn attributes_inside_macro_groups() {
        let src = "m! {\n    mod x {\n        /// docs\n        todo!()\n    }\n}";
        assert_eq!(
            lint_rule(super::NAME, src),
            vec!["4: todo_message"],
            "one diagnostic, on the macro's own line"
        );
    }

    #[test]
    fn unimplemented() {
        assert_eq!(
            lint_rule(super::NAME, "fn f() { unimplemented!() }"),
            vec!["1: todo_message"]
        );
        assert_eq!(
            lint_rule(super::NAME, "fn f() { std::unimplemented!() }"),
            vec!["1: todo_message"]
        );
        assert_eq!(
            lint_rule(super::NAME, "fn f() { unimplemented!(\"views\") }"),
            Vec::<String>::new()
        );
        assert_eq!(
            lint_rule(super::NAME, "fn f() { my::todo!() }"),
            Vec::<String>::new(),
            "not std's"
        );
    }
}
