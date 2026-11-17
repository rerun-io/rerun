//! `log_sensitive_inline`: pass URLs and paths to log macros as structured fields.
//!
//! Structured fields can be stripped from logs and analytics; text in the message cannot.

use super::{Rule, macro_invocations};
use crate::{
    diagnostic::Diagnostic,
    format_args::{ArgRef, FormatCall},
    source::SourceFile,
};

const NAME: &str = "log_sensitive_inline";

/// Warning and error logging macros, from `re_log`, `tracing`, or `log`, or imported from one of them.
const MACROS: &[&str] = &["error", "error_once", "warn", "warn_once"];

const LOG_CRATES: &[&str] = &["", "log", "re_log", "tracing"];

const SENSITIVE_WORDS: &[&str] = &[
    "dir",
    "directory",
    "file",
    "file_path",
    "filename",
    "filepath",
    "folder",
    "path",
    "uri",
    "url",
];

pub const RULE: Rule = Rule {
    name: NAME,
    check,
    check_markdown: true,
    check_tests: true,
};

fn check(file: &SourceFile, diagnostics: &mut Vec<Diagnostic>) {
    for invocation in macro_invocations(file) {
        let (krate, name) = invocation
            .path
            .rsplit_once("::")
            .unwrap_or(("", invocation.path.as_str()));
        if !MACROS.contains(&name) || !LOG_CRATES.contains(&krate) {
            continue;
        }
        let Some(format_call) = FormatCall::parse(&invocation.path, &invocation.token_tree) else {
            continue;
        };
        for placeholder in &format_call.placeholders {
            if let ArgRef::Name(name) = &placeholder.arg
                && is_sensitive(name)
            {
                diagnostics.push(Diagnostic::new(
                    NAME,
                    invocation.offset,
                    format!(
                        "URLs and paths should be passed as structured fields, not inline in log messages. \
                         Use e.g. `re_log::warn!(?{name}, \"message: {{err}}\")` instead of `re_log::warn!(\"message {{{name}}}: {{err}}\")`"
                    ),
                ));
            }
        }
    }
}

/// `url`, `url_str`, and `file_path` are sensitive. `dirty`, `directive`, and `entity_path` are not.
fn is_sensitive(name: &str) -> bool {
    let first_word = name.split('_').next().unwrap_or_default();
    SENSITIVE_WORDS
        .iter()
        .any(|word| name == *word || first_word == *word)
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint_rule;

    fn lint(src: &str) -> Vec<String> {
        lint_rule(super::NAME, src)
    }

    #[test]
    fn inline() {
        assert_eq!(
            lint(r#"fn f() { re_log::warn!("Failed to open {url}: {err}"); }"#),
            vec!["1: log_sensitive_inline"]
        );
        assert_eq!(
            lint(
                "fn f() {\n    re_log::error_once!(\n        \"Failed to load {file_path:?}\"\n    );\n}"
            ),
            vec!["2: log_sensitive_inline"]
        );
        assert_eq!(
            lint(r#"fn f() { re_log::warn!(?url, "Failed to open: {err}"); }"#),
            Vec::<String>::new()
        );
        assert_eq!(
            lint(r#"fn f() { re_log::info!("Opened {url}"); }"#),
            Vec::<String>::new()
        );
    }

    #[test]
    fn positional_args_are_fine() {
        // Our convention is to put the path last, e.g. `"Failed: {err}\nFile path: {}", path.display()`.
        assert_eq!(
            lint(r#"fn f() { re_log::warn!("Failed: {err}\nFile path: {}", path.display()); }"#),
            Vec::<String>::new()
        );
    }

    #[test]
    fn whole_words() {
        assert_eq!(
            lint(r#"fn f() { re_log::warn!("bad {url_str}"); }"#),
            vec!["1: log_sensitive_inline"]
        );
        assert_eq!(
            lint(r#"fn f() { ::re_log::warn!("bad {path}"); }"#),
            vec!["1: log_sensitive_inline"]
        );
        assert_eq!(
            lint(r#"fn f() { re_log::warn!("{dirty} {directive} {entity_path}"); }"#),
            Vec::<String>::new()
        );
    }

    #[test]
    fn other_log_crates() {
        assert_eq!(
            lint(r#"fn f() { tracing::warn!("bad {path}"); }"#),
            vec!["1: log_sensitive_inline"]
        );
        assert_eq!(
            lint(r#"fn f() { error!("bad {url}"); }"#),
            vec!["1: log_sensitive_inline"]
        );
        assert_eq!(
            lint(r#"fn f() { my::warn!("bad {url}"); }"#),
            Vec::<String>::new()
        );
    }
}
