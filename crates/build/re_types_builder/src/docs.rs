use crate::codegen::Target;

/// A high-level representation of the contetns of a flatbuffer docstring.
#[derive(Debug, Clone)]
pub struct Docs {
    /// All docmentation lines, including the leading tag, if any.
    ///
    /// If the tag is the empty string, it means the line is untagged.
    ///
    /// Each line excludes the leading space and trailing newline.
    /// * `/// COMMENT\n`      =>  `("", "COMMENT")`
    /// * `/// \py COMMENT\n`  =>  `("py", "COMMENT")`.
    lines: Vec<(String, String)>,
}

impl Docs {
    pub fn from_raw_docs(
        docs: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<&'_ str>>>,
    ) -> Self {
        Self::from_lines(docs.into_iter().flat_map(|doc| doc.into_iter()))
    }

    pub fn from_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Self {
        let lines: Vec<(String, String)> = lines.map(parse_line).collect();

        for (tag, comment) in &lines {
            assert!(is_known_tag(tag), "Unknown tag: '\\{tag} {comment}'");
        }

        Self { lines }
    }

    /// Get the first line of the documentation untagged.
    pub fn first_line(&self) -> Option<String> {
        let (tag, line) = self.lines.first()?;
        assert!(
            tag.is_empty(),
            "Expected no tag on first line of docstring. Found: /// \\{tag} {line}"
        );
        Some(line.to_owned())
    }

    /// Get all doc lines that start with the given tag.
    ///
    /// For instance, pass `"example"` to get all lines that start with `"\example"`.
    pub fn only_lines_tagged(&self, tag: &str) -> Vec<&str> {
        assert!(is_known_tag(tag), "Unknown tag: '{tag}'");
        self.lines
            .iter()
            .filter_map(
                |(t, line)| {
                    if t == tag {
                        Some(line.as_str())
                    } else {
                        None
                    }
                },
            )
            .collect()
    }

    /// Get all doc lines that are untagged, or match the given tag.
    ///
    /// For instance, pass `"py"` to get all lines that are untagged or starts with `"\py"`.
    pub fn lines_including_tag(&self, tag: &str) -> Vec<String> {
        assert!(is_known_tag(tag), "Unknown tag: '{tag}'");
        remove_extra_newlines(self.lines.iter().filter_map(|(t, line)| {
            if t.is_empty() || t == tag {
                Some(line.to_owned())
            } else {
                None
            }
        }))
    }
}

fn remove_extra_newlines(lines: impl Iterator<Item = String>) -> Vec<String> {
    let mut lines: Vec<String> = lines.collect();

    // Remove duplicated blank lines:
    lines.dedup();

    // Remove trailing blank lines:
    while let Some(line) = lines.last() {
        if line.is_empty() {
            lines.pop();
        } else {
            break;
        }
    }

    lines
}

fn is_known_tag(tag: &str) -> bool {
    matches!(tag, "" | "example" | "cpp" | "py" | "rs" | "md")
}

/// Parses `" \tag The comment"` into `("tag", "The comment")`.
///
/// `" The comment"` becomes `("", "The comment")`.
fn parse_line(line: &str) -> (String, String) {
    if let Some(line) = line.strip_prefix(" \\") {
        // \tagged comment
        let tag = line.split_whitespace().next().unwrap().to_owned();
        let line = &line[tag.len()..];
        if let Some(line) = line.strip_prefix(' ') {
            // Removed space between tag and comment.
            (tag, line.to_owned())
        } else {
            assert!(line.is_empty());
            (tag, String::new())
        }
    } else if let Some(line) = line.strip_prefix(' ') {
        // Removed space between `///` and comment.
        (String::new(), line.to_owned())
    } else {
        assert!(
            line.is_empty(),
            "Comments should start with a single space; found {line:?}"
        );
        (String::new(), String::new())
    }
}

/// We support doclinks in our docstrings.
///
/// They need to follow this format:
/// ```fbs
/// /// See also [archetype.Image].
/// table Tensor { … }
/// ```
///
/// This module is all about translating these doclinks to the different [`Target`]s.
///
/// The code is not very efficient, but it is simple and works.
mod doclink_translation {
    use super::Target;

    /// Convert Rerun-style doclinks to the target language.
    ///
    /// For example, "[archetype.Image]" becomes "[`archetype::Image`]" in Rust.
    pub fn translate_doc_line(input: &str, target: Target) -> String {
        let mut out_tokens: Vec<String> = vec![];
        let mut within_backticks = false;

        let mut tokens = tokenize(input).into_iter().peekable();
        while let Some(token) = tokens.next() {
            if token == "`" {
                within_backticks = !within_backticks;
                out_tokens.push(token.to_owned());
                continue;
            }

            if within_backticks {
                out_tokens.push(token.to_owned());
                continue;
            }

            if token == "[" {
                // Potential start of a Rerun doclink
                let mut doclink_tokens = vec![token];
                for token in &mut tokens {
                    doclink_tokens.push(token);
                    if token == "]" {
                        break;
                    }
                }

                if tokens
                    .peek()
                    .map_or(false, |next_token| next_token.starts_with('('))
                {
                    // We are at the `)[` boundary of a markdown link, e.g. "[Rerun](https://rerun.io)",
                    // so this is not a rerun doclink after all.
                    out_tokens.extend(doclink_tokens.iter().map(|&s| s.to_owned()));
                    continue;
                }

                out_tokens.push(translate_doclink_or_die(&doclink_tokens, target));
                continue;
            }

            // Normal boring token
            out_tokens.push(token.to_owned());
        }

        out_tokens.into_iter().collect()
    }

    fn translate_doclink_or_die(doclink_tokens: &[&str], target: Target) -> String {
        translate_doclink(doclink_tokens, target).unwrap_or_else(|err| {
            let original_doclink: String = doclink_tokens.join("");
            panic!("Failed to parse the doclink '{original_doclink}': {err}");
        })
    }

    fn translate_doclink(doclink_tokens: &[&str], target: Target) -> Result<String, &'static str> {
        let original_doclink: String = doclink_tokens.join("");
        let mut tokens = doclink_tokens.iter();
        if tokens.next() != Some(&"[") {
            return Err("Missing opening bracket");
        }
        let kind = tokens.next().ok_or("Missing kind")?;
        if tokens.next() != Some(&".") {
            return Err("Missing dot");
        }
        let name = tokens.next().ok_or("Missing name")?;
        if tokens.next() != Some(&"]") {
            return Err("Missing closing bracket");
        }
        if tokens.next().is_some() {
            return Err("Trailing tokens");
        }

        Ok(match target {
            Target::Cpp => format!("[`rerun::{kind}::{name}`]"),
            Target::Rust => {
                // https://doc.rust-lang.org/rustdoc/write-documentation/linking-to-items-by-name.html
                format!("[`{kind}::{name}`][crate::{kind}::{name}]")
            }
            Target::Python => format!("[`{kind}.{name}`][rerun.{kind}.{name}]"),
            Target::Docs => {
                // For instance, https://rerun.io/docs/reference/types/views/spatial2d_view
                let mame_snake_case = re_case::to_snake_case(name);
                format!(
                "[`{kind}.{name}`](https://rerun.io/docs/reference/types/{kind}/{mame_snake_case})"
            )
            }
        })
    }

    fn tokenize(mut input: &str) -> Vec<&str> {
        tokenize_with(input, &['[', ']', '`', '.'])
    }

    fn tokenize_with<'input>(mut input: &'input str, special_chars: &[char]) -> Vec<&'input str> {
        let mut tokens = vec![];
        while !input.is_empty() {
            if let Some(index) = input.find(|c| special_chars.contains(&c)) {
                if 0 < index {
                    tokens.push(&input[..index]);
                }
                tokens.push(&input[index..index + 1]);
                input = &input[index + 1..];
            } else {
                tokens.push(input);
                break;
            }
        }
        tokens
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_tokenize() {
            assert_eq!(tokenize("This is a comment"), vec!["This is a comment"]);
            assert_eq!(
                tokenize("A vector `[1, 2, 3]` and a doclink [archetype.Image]."),
                vec![
                    "A vector ",
                    "`",
                    "[",
                    "1, 2, 3",
                    "]",
                    "`",
                    " and a doclink ",
                    "[",
                    "archetype",
                    ".",
                    "Image",
                    "]",
                    "."
                ]
            );
        }

        #[test]
        fn test_translate_doclinks() {
            let input =
                "A vector `[1, 2, 3]` and a doclink [views.Spatial2DView] and a [url](www.rerun.io).";

            assert_eq!(
                translate_doc_line(
                    input,
                    Target::Cpp
                ),
                "A vector `[1, 2, 3]` and a doclink [`rerun::views::Spatial2DView`] and a [url](www.rerun.io)."
            );

            assert_eq!(
                translate_doc_line(
                    input,
                    Target::Python
                ),
                "A vector `[1, 2, 3]` and a doclink [`views.Spatial2DView`][rerun.views.Spatial2DView] and a [url](www.rerun.io)."
            );

            assert_eq!(
                translate_doc_line(
                    input,
                    Target::Rust
                ),
                "A vector `[1, 2, 3]` and a doclink [`views::Spatial2DView`][crate::views::Spatial2DView] and a [url](www.rerun.io)."
            );

            assert_eq!(
                translate_doc_line(
                    input,
                    Target::Docs
                ),
                "A vector `[1, 2, 3]` and a doclink [`views.Spatial2DView`](https://rerun.io/docs/reference/types/views/spatial2d_view) and a [url](www.rerun.io)."
            );
        }
    }
}

use doclink_translation::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docs() {
        let docs = Docs::from_lines(
            [
                r" The first line.",
                r" ",
                r" The second line.",
                r" ",
                r" \py Only for Python.",
                r" ",
                r" The third line.",
                r" ",
                r" \cpp Only for C++.",
            ]
            .into_iter(),
        );

        assert_eq!(docs.only_lines_tagged("py"), vec!["Only for Python.",]);

        assert_eq!(docs.only_lines_tagged("cpp"), vec!["Only for C++.",]);

        assert_eq!(
            docs.lines_including_tag("py"),
            vec![
                "The first line.",
                "",
                "The second line.",
                "",
                "Only for Python.",
                "",
                "The third line.",
            ]
        );

        assert_eq!(
            docs.lines_including_tag("cpp"),
            vec![
                "The first line.",
                "",
                "The second line.",
                "",
                "The third line.",
                "",
                "Only for C++.",
            ]
        );

        assert_eq!(docs.first_line(), Some("The first line.".to_owned()));
    }
}
