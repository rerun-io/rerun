//! Linting the Rust code blocks in markdown files and in doc comments.
//!
//! We make a "view" of the file: a copy of the same length, with the same newlines,
//! in which everything but the code is blanked out with spaces.
//! Because offsets in the view are offsets in the file, diagnostics need no mapping.

use std::ops::Range;

use ra_ap_syntax::TextSize;

use crate::{diagnostic::Diagnostic, rules, source::SourceFile, suppression};

/// The rules and markers to apply to code blocks.
pub struct Context<'a> {
    pub path: &'a str,

    /// The enabled rules that apply to code blocks.
    pub rules: &'a [&'a rules::Rule],

    /// All rule names, for `relint: allow(…)` markers.
    pub known_rules: &'a [&'a str],

    /// Rules that are not checked in code blocks, for `relint: allow(…)` markers.
    pub unchecked_rules: &'a [String],
}

/// Lint the ```` ```rust ```` code blocks of a markdown file.
pub fn lint_markdown(context: &Context<'_>, text: &str) -> Vec<Diagnostic> {
    let view = blank_blockquote_markers(text);
    let blocks = fenced_blocks(&view, 3, |language| {
        matches!(language.to_lowercase().as_str(), "rust" | "rs")
    });
    lint_blocks(context, &view, &blocks)
}

/// Lint the code blocks in the `///` and `//!` doc comments of a Rust file.
///
/// As in rustdoc, a code block without a language is Rust.
pub fn lint_doc_comments(context: &Context<'_>, text: &str) -> Vec<Diagnostic> {
    let mut view = doc_comment_view(text);
    let blocks = fenced_blocks(&view, usize::MAX, |language| {
        language.is_empty()
            || language == "rust"
            || matches!(
                language,
                "ignore" | "no_run" | "should_panic" | "compile_fail" | "test_harness"
            )
            || language.starts_with("edition")
    });
    blank_hidden_rustdoc_lines(&mut view, &blocks);
    lint_blocks(context, &view, &blocks)
}

/// Lint each block as a function body, with the markers in that block only.
fn lint_blocks(context: &Context<'_>, view: &str, blocks: &[Range<usize>]) -> Vec<Diagnostic> {
    const PREFIX: &str = "fn _(){";

    let mut diagnostics = Vec::new();
    for block in blocks {
        let Ok(start) = u32::try_from(block.start).map(TextSize::from) else {
            continue;
        };
        let body = SourceFile::parse(
            context.path.to_owned(),
            format!("{PREFIX}{}\n}}", &view[block.clone()]),
        );
        let block_diagnostics: Vec<Diagnostic> = rules::check_file(&body, context.rules)
            .into_iter()
            .filter_map(|diagnostic| diagnostic.shifted(TextSize::of(PREFIX), start))
            .collect();

        // Markers are found in a view of this block alone, so that an unterminated string
        // in one block can't hide the markers in the next.
        let block_view = SourceFile::parse(
            context.path.to_owned(),
            mask(view, std::slice::from_ref(block)),
        );
        diagnostics.extend(suppression::apply_markers(
            &block_view,
            block_diagnostics,
            context.known_rules,
            context.unchecked_rules,
        ));
    }
    diagnostics
}

/// An open fence, while looking for the matching closing fence.
struct OpenFence<'a> {
    /// The fence characters, e.g. ```` ``` ```` or `~~~~`.
    fence: &'a str,

    /// Is this a block we lint?
    is_rust: bool,

    /// Where the contents start.
    contents_start: usize,
}

/// The byte ranges of the contents of the fenced code blocks for which `is_rust(language)` is true.
///
/// A fence may be indented by at most `max_indent` spaces.
/// A block that is never closed runs to the end of the text, as in `CommonMark`.
fn fenced_blocks(
    view: &str,
    max_indent: usize,
    is_rust: impl Fn(&str) -> bool,
) -> Vec<Range<usize>> {
    let mut blocks = Vec::new();
    let mut open: Option<OpenFence<'_>> = None;
    let mut offset = 0;
    for line in view.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        let fence_len = trimmed
            .chars()
            .take_while(|&c| c == '`' || c == '~')
            .count();
        let fence = &trimmed[..fence_len];
        let is_fence =
            3 <= fence_len && indent <= max_indent && fence.chars().all(|c| fence.starts_with(c));
        let info = trimmed[fence_len..].trim();
        match &open {
            None if is_fence => {
                let language = info
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .next()
                    .unwrap_or_default();
                open = Some(OpenFence {
                    fence,
                    is_rust: is_rust(language),
                    contents_start: offset + line.len(),
                });
            }
            Some(open_fence)
                if is_fence && info.is_empty() && fence.starts_with(open_fence.fence) =>
            {
                if open_fence.is_rust {
                    blocks.push(open_fence.contents_start..offset);
                }
                open = None;
            }
            _ => {}
        }
        offset += line.len();
    }
    if let Some(open_fence) = open
        && open_fence.is_rust
    {
        blocks.push(open_fence.contents_start..view.len());
    }
    blocks
}

/// `text` with the `>` markers of blockquotes replaced by spaces.
fn blank_blockquote_markers(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let quoted = line.trim_start_matches(['>', ' ', '\t']);
            if quoted.len() < line.len() && line[..line.len() - quoted.len()].contains('>') {
                " ".repeat(line.len() - quoted.len()) + quoted
            } else {
                line.to_owned()
            }
        })
        .collect()
}

/// `text` with everything but the contents of `///` and `//!` doc comments replaced by spaces.
fn doc_comment_view(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();
            let is_doc = (trimmed.starts_with("///") && !trimmed.starts_with("////"))
                || trimmed.starts_with("//!");
            if is_doc {
                " ".repeat(indent + 3) + &trimmed[3..]
            } else {
                blank(line)
            }
        })
        .collect()
}

/// Blank the `#` of rustdoc's hidden lines (`# use foo;`) in the code blocks.
fn blank_hidden_rustdoc_lines(view: &mut String, blocks: &[Range<usize>]) {
    let mut hidden = Vec::new();
    for block in blocks {
        let mut offset = block.start;
        for line in view[block.clone()].split_inclusive('\n') {
            let trimmed = line.trim_start();
            if trimmed.trim_end() == "#" || trimmed.starts_with("# ") {
                hidden.push(offset + line.len() - trimmed.len());
            }
            offset += line.len();
        }
    }
    for offset in hidden {
        view.replace_range(offset..=offset, " ");
    }
}

/// Spaces, keeping the newline.
fn blank(line: &str) -> String {
    line.bytes()
        .map(|byte| if byte == b'\n' { '\n' } else { ' ' })
        .collect()
}

/// `text` with everything outside `blocks` replaced by spaces, keeping newlines and byte offsets.
fn mask(text: &str, blocks: &[Range<usize>]) -> String {
    let mut masked: Vec<u8> = text
        .bytes()
        .map(|byte| if byte == b'\n' { b'\n' } else { b' ' })
        .collect();
    for block in blocks {
        masked[block.clone()].copy_from_slice(&text.as_bytes()[block.clone()]);
    }
    String::from_utf8_lossy(&masked).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context_rules() -> (Vec<&'static rules::Rule>, Vec<&'static str>, Vec<String>) {
        let rules: Vec<&rules::Rule> = rules::RULES
            .iter()
            .filter(|rule| rule.check_markdown)
            .collect();
        let known: Vec<&str> = rules::RULES.iter().map(|rule| rule.name).collect();
        let unchecked: Vec<String> = rules::RULES
            .iter()
            .filter(|rule| !rule.check_markdown)
            .map(|rule| rule.name.to_owned())
            .collect();
        (rules, known, unchecked)
    }

    fn lint_with(text: &str, lint: fn(&Context<'_>, &str) -> Vec<Diagnostic>) -> Vec<String> {
        let (rules, known, unchecked) = context_rules();
        let context = Context {
            path: "test",
            rules: &rules,
            known_rules: &known,
            unchecked_rules: &unchecked,
        };
        let file = SourceFile::parse("test".to_owned(), text.to_owned());
        let mut result: Vec<String> = lint(&context, text)
            .into_iter()
            .map(|diagnostic| format!("{}: {}", file.line(diagnostic.offset) + 1, diagnostic.rule))
            .collect();
        result.sort();
        result
    }

    fn lint_md(text: &str) -> Vec<String> {
        lint_with(text, lint_markdown)
    }

    fn lint_docs(text: &str) -> Vec<String> {
        lint_with(text, lint_doc_comments)
    }

    #[test]
    fn markdown_blocks() {
        let md =
            "# Title\n\nif let Err(e) = x {}\n\n```rust\nlet x = 1;\nif let Err(e) = x {}\n```\n";
        assert_eq!(lint_md(md), vec!["7: error_name"], "prose is not linted");

        let md = "```rs,ignore\nx.map_err(|e| e);\n```\n\n```py\nx.map_err(|e| e)\n```\n";
        assert_eq!(lint_md(md), vec!["2: error_name"], "only Rust blocks");

        let md = "- item\n\n  ```rust\n  todo!()\n  ```\n";
        assert_eq!(
            lint_md(md),
            vec!["4: todo_message"],
            "indented blocks in lists"
        );

        let md = "````rust\n```\ntodo!()\n````\n";
        assert_eq!(
            lint_md(md),
            vec!["3: todo_message"],
            "a shorter fence does not close the block"
        );
    }

    #[test]
    fn markdown_fence_details() {
        assert_eq!(
            lint_md("```rs\ntodo!()\n```\n"),
            vec!["2: todo_message"],
            "at the start of the file"
        );
        assert_eq!(
            lint_md("``` Rust\ntodo!()\n```\n"),
            vec!["2: todo_message"],
            "space and casing"
        );
        assert_eq!(
            lint_md("> ```rust\n> todo!()\n> ```\n"),
            vec!["2: todo_message"],
            "in a blockquote"
        );
        assert_eq!(
            lint_md("```rust\ntodo!()\n"),
            vec!["2: todo_message"],
            "never closed"
        );
        assert_eq!(
            lint_md("text\n\n    ```rust\n    todo!()\n    ```\n"),
            Vec::<String>::new(),
            "indented by 4 spaces: an indented code block, not a fence"
        );
    }

    #[test]
    fn markdown_field_counts() {
        assert_eq!(
            lint_md("```rust\nstruct A(u32, String);\n```\n"),
            vec!["2: positional_fields"]
        );
        assert_eq!(
            lint_md("```rust\nenum E {\n    A { a: u8, b: u8, c: u8, d: u8, e: u8 },\n}\n```\n"),
            vec!["3: variant_fields"]
        );
    }

    #[test]
    fn markdown_markers() {
        let md = "```rust\ntodo!() // relint: allow(todo_message): example\nlet rt = tokio::runtime::Runtime::new();\n```\n";
        assert_eq!(
            lint_md(md),
            Vec::<String>::new(),
            "markers work, and non-markdown rules are off"
        );

        let md = "```rust\nlet s = \"unterminated;\n```\n\n```rust\ntodo!() // relint: allow(todo_message): example\n```\n";
        assert_eq!(
            lint_md(md),
            Vec::<String>::new(),
            "markers are found per block"
        );
    }

    #[test]
    fn doc_comments() {
        let src = "/// Example:\n///\n/// ```\n/// x.map_err(|e| e);\n/// ```\nfn f() {}\n";
        assert_eq!(
            lint_docs(src),
            vec!["4: error_name"],
            "a block without a language is Rust"
        );

        let src = "    //! ```no_run\n    //! todo!()\n    //! ```\n";
        assert_eq!(lint_docs(src), vec!["2: todo_message"]);

        let src = "/// ```text\n/// todo!()\n/// ```\n// ```\n// todo!()\n// ```\n";
        assert_eq!(
            lint_docs(src),
            Vec::<String>::new(),
            "not Rust, and not a doc comment"
        );

        let src = "/// ```\n/// # fn main() {\n/// todo!()\n/// # }\n/// ```\n";
        assert_eq!(lint_docs(src), vec!["3: todo_message"], "hidden lines");
    }
}
