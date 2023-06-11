//! Helpers common to all codegen passes.

use crate::Docs;

pub fn quote_doc_from_docs(docs: &Docs, tags: &[&str]) -> Vec<String> {
    fn trim_mono_start_whitespace_if_needed(line: &str) -> &str {
        if line.chars().next().map_or(false, |c| c.is_whitespace()) {
            // NOTE: don't trim! only that very specific space should go away
            &line[1..]
        } else {
            line
        }
    }

    let mut lines = Vec::new();

    for line in &docs.doc {
        lines.push(trim_mono_start_whitespace_if_needed(line).to_owned());
    }

    let empty = Vec::new();
    for tag in tags {
        for line in docs.tagged_docs.get(*tag).unwrap_or(&empty) {
            lines.push(trim_mono_start_whitespace_if_needed(line).to_owned());
        }
    }

    // NOTE: remove duplicated blank lines.
    lines.dedup();

    // NOTE: remove trailing blank lines.
    while let Some(line) = lines.last() {
        if line.is_empty() {
            lines.pop();
        } else {
            break;
        }
    }

    lines
}
