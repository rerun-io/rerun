//! `// relint: allow(rule_name): reason` markers.
//!
//! A marker is a comment that starts with `relint:`.
//! A marker at the end of a line allows the named rules on that line.
//! A marker on a line of its own allows them on the next line that is not a comment.
//!
//! Every rule named in a marker must suppress at least one diagnostic,
//! so that markers do not outlive the code they were written for.
//! Every marker must say why, after the rule list.

use ra_ap_syntax::{AstToken as _, SyntaxKind, TextRange, TextSize, ast};

use crate::{diagnostic::Diagnostic, source::SourceFile};

/// The name of the diagnostics about the markers themselves.
pub const UNUSED_ALLOW: &str = "unused_allow";

const PREFIX: &str = "relint:";

struct Marker {
    /// Start of the comment.
    offset: TextSize,

    /// The 0-based line whose diagnostics the marker allows.
    target_line: usize,

    /// The allowed rules, and whether each one suppressed something.
    rules: Vec<(String, bool)>,
}

/// Remove the diagnostics allowed by markers, and add diagnostics for bad markers.
///
/// `known_rules` are the names of all rules, including the `disabled_rules`: those not checked in this file.
pub fn apply_markers(
    file: &SourceFile,
    diagnostics: Vec<Diagnostic>,
    known_rules: &[&str],
    disabled_rules: &[String],
) -> Vec<Diagnostic> {
    let (mut markers, mut problems) = find_markers(file);
    let mut result: Vec<Diagnostic> = diagnostics
        .into_iter()
        .filter(|diagnostic| {
            let line = file.line(diagnostic.offset);
            let mut allowed = false;
            for marker in markers.iter_mut().filter(|m| m.target_line == line) {
                for (rule, used) in &mut marker.rules {
                    if rule == diagnostic.rule {
                        *used = true;
                        allowed = true;
                    }
                }
            }
            !allowed
        })
        .collect();

    for marker in &markers {
        for (rule, used) in &marker.rules {
            if !known_rules.contains(&rule.as_str()) {
                result.push(Diagnostic::new(
                    UNUSED_ALLOW,
                    marker.offset,
                    format!("Unknown rule {rule:?} in `relint: allow(…)`"),
                ));
            } else if disabled_rules.contains(rule) {
                result.push(Diagnostic::new(
                    UNUSED_ALLOW,
                    marker.offset,
                    format!(
                        "`relint: allow({rule})` is not needed: `{rule}` is not checked in this file (it is disabled in `{}`, or does not apply to markdown)",
                        crate::config::FILE_NAME
                    ),
                ));
            } else if !used {
                result.push(Diagnostic::new(
                    UNUSED_ALLOW,
                    marker.offset,
                    format!(
                        "`relint: allow({rule})` does not suppress anything. Remove it, or move it to the line the diagnostic is on."
                    ),
                ));
            }
        }
    }

    result.append(&mut problems);
    result
}

/// The markers in a file, and diagnostics for malformed ones.
fn find_markers(file: &SourceFile) -> (Vec<Marker>, Vec<Diagnostic>) {
    let mut markers = Vec::new();
    let mut problems = Vec::new();
    let comments = file
        .syntax()
        .descendants_with_tokens()
        .filter_map(ra_ap_syntax::NodeOrToken::into_token)
        .filter(|token| token.kind() == SyntaxKind::COMMENT)
        .filter_map(ast::Comment::cast)
        .filter(|comment| !comment.is_doc());
    for comment in comments {
        let text = comment.text();
        let body = text
            .strip_prefix("//")
            .or_else(|| {
                text.strip_prefix("/*")
                    .map(|rest| rest.trim_end_matches("*/"))
            })
            .unwrap_or(text)
            .trim();
        if !body.starts_with(PREFIX) {
            continue;
        }
        let range = comment.syntax().text_range();
        match parse_marker(body) {
            Ok(rules) => markers.push(Marker {
                offset: range.start(),
                target_line: target_line(file, range),
                rules: rules.into_iter().map(|rule| (rule, false)).collect(),
            }),
            Err(err) => problems.push(Diagnostic::new(
                UNUSED_ALLOW,
                range.start(),
                err.to_string(),
            )),
        }
    }
    (markers, problems)
}

/// What is wrong with a `relint: allow(…)` marker.
#[derive(Debug, thiserror::Error)]
enum MarkerError {
    #[error("Malformed marker. {SYNTAX}")]
    Malformed,

    #[error("Malformed marker: missing `)`. {SYNTAX}")]
    MissingParen,

    #[error("Marker without rules. {SYNTAX}")]
    NoRules,

    #[error("Say why the rule does not apply here. {SYNTAX}")]
    NoReason,
}

const SYNTAX: &str = "Write markers as `// relint: allow(rule_name): reason`";

/// Parse `relint: allow(rule_a, rule_b): reason` into the rule names.
fn parse_marker(body: &str) -> Result<Vec<String>, MarkerError> {
    let list = body
        .strip_prefix("relint: allow(")
        .ok_or(MarkerError::Malformed)?;
    let (list, rest) = list.split_once(')').ok_or(MarkerError::MissingParen)?;
    let rules: Vec<String> = list
        .split(',')
        .map(str::trim)
        .filter(|rule| !rule.is_empty())
        .map(str::to_owned)
        .collect();
    if rules.is_empty() {
        return Err(MarkerError::NoRules);
    }
    let reason = rest
        .trim()
        .strip_prefix(':')
        .map(str::trim)
        .unwrap_or_default();
    if reason.is_empty() {
        return Err(MarkerError::NoReason);
    }
    Ok(rules)
}

fn target_line(file: &SourceFile, comment: TextRange) -> usize {
    let line = file.line(comment.start());
    let line_end = file.line_start(line + 1).max(usize::from(comment.end()));
    let code_before = &file.text[file.line_start(line)..usize::from(comment.start())];
    let code_after = &file.text[usize::from(comment.end())..line_end];
    let is_trailing = !code_before.trim().is_empty() || !code_after.trim().is_empty();
    if is_trailing {
        return line;
    }
    (line + 1..file.num_lines())
        .find(|&next| !file.line_text(next).trim_start().starts_with("//"))
        .unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::lint;

    #[test]
    fn trailing_marker() {
        let src = "fn f() { x.zip(y); } // relint: allow(zip_chain_method): test\n";
        assert_eq!(lint(src), Vec::<String>::new());
    }

    #[test]
    fn marker_on_line_above() {
        let src = "fn f() {\n    // relint: allow(zip_chain_method): test\n    // other comment\n    x.zip(y);\n}\n";
        assert_eq!(lint(src), Vec::<String>::new());
    }

    #[test]
    fn marker_only_covers_one_line() {
        let src = "fn f() {\n    // relint: allow(zip_chain_method): test\n    x.zip(y);\n    x.zip(y);\n}\n";
        assert_eq!(lint(src), vec!["4: zip_chain_method"]);
    }

    #[test]
    fn unused_marker() {
        let src = "fn f() {} // relint: allow(zip_chain_method): test\n";
        assert_eq!(lint(src), vec!["1: unused_allow"]);
    }

    #[test]
    fn unknown_rule() {
        let src = "fn f() { x.zip(y); } // relint: allow(zip_chain_method, no_such_rule): test\n";
        assert_eq!(lint(src), vec!["1: unused_allow"]);
    }

    #[test]
    fn block_comment_marker_before_code() {
        let src = "fn f() {\n    /* relint: allow(zip_chain_method): test */ x.zip(y);\n    x.zip(y);\n}\n";
        assert_eq!(lint(src), vec!["3: zip_chain_method"]);
    }

    #[test]
    fn marker_in_string_is_ignored() {
        let src = "fn f() { x.zip(\"// relint: allow(zip_chain_method): test\"); }\n";
        assert_eq!(lint(src), vec!["1: zip_chain_method"]);
    }

    #[test]
    fn doc_comment_is_not_a_marker() {
        let src = "/// relint: allow(zip_chain_method): test\nfn f() { x.zip(y); }\n";
        assert_eq!(lint(src), vec!["2: zip_chain_method"]);
    }

    #[test]
    fn marker_for_disabled_rule() {
        let file = crate::source::SourceFile::parse(
            "test.rs".to_owned(),
            "fn f() {} // relint: allow(tokio_runtime): test\n".to_owned(),
        );
        let diagnostics = super::apply_markers(
            &file,
            vec![],
            &["tokio_runtime"],
            &["tokio_runtime".to_owned()],
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("disabled"));
    }

    #[test]
    fn marker_syntax() {
        let src = "fn f() { x.zip(y); } // relint: allow(zip_chain_method)\n";
        assert_eq!(
            lint(src),
            vec!["1: unused_allow", "1: zip_chain_method"],
            "a reason is required"
        );

        let src = "fn f() { x.zip(y); } // relint: allow(zip_chain_method: forgot the paren\n";
        assert_eq!(
            lint(src),
            vec!["1: unused_allow", "1: zip_chain_method"],
            "malformed"
        );

        let src = "fn f() { x.zip(y); } // relint:allow(zip_chain_method): no space\n";
        assert_eq!(
            lint(src),
            vec!["1: unused_allow", "1: zip_chain_method"],
            "malformed"
        );

        let src = "// See `// relint: allow(rule_name): reason` in the README.\nfn f() {}\n";
        assert_eq!(lint(src), Vec::<String>::new(), "a mention is not a marker");
    }
}
