use crate::{codegen::Target, Objects, Reporter};

/// A high-level representation of the contents of a flatbuffer docstring.
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
        reporter: &Reporter,
        virtpath: &str,
        fqname: &str,
        docs: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<&'_ str>>>,
    ) -> Self {
        Self::from_lines(
            reporter,
            virtpath,
            fqname,
            docs.into_iter().flat_map(|doc| doc.into_iter()),
        )
    }

    pub fn from_lines<'a>(
        reporter: &Reporter,
        virtpath: &str,
        fqname: &str,
        lines: impl Iterator<Item = &'a str>,
    ) -> Self {
        let lines: Vec<(String, String)> = lines.map(parse_line).collect();

        for (tag, comment) in &lines {
            assert!(is_known_tag(tag), "Unknown tag: '\\{tag} {comment}'");

            if tag.is_empty() {
                find_and_recommend_doclinks(reporter, virtpath, fqname, comment);
            }
        }

        Self { lines }
    }

    /// Get the first line of the documentation untagged.
    pub fn first_line(
        &self,
        reporter: &Reporter,
        objects: &Objects,
        target: Target,
    ) -> Option<String> {
        let (tag, line) = self.lines.first()?;
        assert!(
            tag.is_empty(),
            "Expected no tag on first line of docstring. Found: /// \\{tag} {line}"
        );
        Some(translate_doc_line(reporter, objects, line, target))
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

    /// Get all doc lines that are untagged, or has a tag matching the given target.
    ///
    /// For instance, pass [`Target::Python`] to get all lines that are untagged or starts with `"\py"`.
    ///
    /// The tagged lines (`\py`) are left as is, but untagged lines will have Rerun doclinks translated to the target language.
    pub(super) fn lines_for(
        &self,
        reporter: &Reporter,
        objects: &Objects,
        target: Target,
    ) -> Vec<String> {
        let target_tag = match target {
            Target::Cpp => "cpp",
            Target::Python => "py",
            Target::Rust => "rs",
            Target::WebDocsMarkdown => "md",
        };
        assert!(
            is_known_tag(target_tag),
            "Unknown target tag: '{target_tag}'"
        );

        remove_extra_newlines(self.lines.iter().filter_map(|(tag, line)| {
            if tag.is_empty() {
                Some(translate_doc_line(reporter, objects, line, target))
            } else if tag == target_tag {
                // We don't expect doclinks in tagged lines, because tagged lines are usually
                // language-specific, and thus should have the correct format already.
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

/// Look for things that look like doclinks to other types, but aren't in brackets.
fn find_and_recommend_doclinks(
    reporter: &Reporter,
    virtpath: &str,
    fqname: &str,
    full_comment: &str,
) {
    let mut comment = full_comment;
    while let Some(start) = comment.find('`') {
        comment = &comment[start + 1..];
        if let Some(end) = comment.find('`') {
            let content = &comment[..end];

            let looks_like_type_name = content.len() > 5
                && content.chars().all(|c| c.is_ascii_alphanumeric())
                && content.chars().next().unwrap().is_ascii_uppercase()

                // TODO(emilk): support references to things outside the default `rerun.scope`.
                && !matches!(content, "SpaceViewContents" | "VisibleTimeRanges" | "QueryExpression")

                // In some blueprint code we refer to stuff in Rerun.
                && !matches!(content, "ChunkStore" | "ContainerId" | "EntityPathFilter" | "Spatial2DView" | "SpaceViewId" | "SpaceView")

                // TODO(emilk): allow doclinks to enum variants.
                && !matches!(content, "Horizontal" | "Vertical" | "SolidColor");

            if looks_like_type_name {
                reporter.warn(virtpath, fqname, format!("`{content}` can be written as a doclink, e.g. [archetypes.{content}] in comment: /// {full_comment}"));
            }
            comment = &comment[end + 1..];
        } else {
            return;
        }
    }
}

use doclink_translation::translate_doc_line;

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
    use crate::{Objects, Reporter};

    use super::Target;

    /// Convert Rerun-style doclinks to the target language.
    pub fn translate_doc_line(
        reporter: &Reporter,
        objects: &Objects,
        input: &str,
        target: Target,
    ) -> String {
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

                out_tokens.push(translate_doclink(
                    reporter,
                    objects,
                    &doclink_tokens,
                    target,
                ));
                continue;
            }

            // Normal boring token
            out_tokens.push(token.to_owned());
        }

        out_tokens.into_iter().collect()
    }

    fn translate_doclink(
        reporter: &Reporter,
        objects: &Objects,
        doclink_tokens: &[&str],
        target: Target,
    ) -> String {
        try_translate_doclink(objects, doclink_tokens, target).unwrap_or_else(|err| {
            let original_doclink: String = doclink_tokens.join("");

            // The worlds simplest heuristic, but at least it doesn't warn about things like [x, y, z, w].
            let looks_like_rerun_doclink =
                !original_doclink.contains(' ') && original_doclink.len() > 6;

            if looks_like_rerun_doclink {
                reporter.warn_no_context(format!(
                    "Looks like a Rerun doclink, but fails to parse: {original_doclink} - {err}"
                ));
            }

            original_doclink
        })
    }

    fn try_translate_doclink(
        objects: &Objects,
        doclink_tokens: &[&str],
        target: Target,
    ) -> Result<String, &'static str> {
        let mut tokens = doclink_tokens.iter();
        if tokens.next() != Some(&"[") {
            return Err("Missing opening bracket");
        }
        let kind = *tokens.next().ok_or("Missing kind")?;
        if kind == "`" {
            return Err("Do not use backticks inside doclinks");
        }
        if tokens.next() != Some(&".") {
            return Err("Missing dot");
        }
        let type_name = *tokens.next().ok_or("Missing type name")?;
        if tokens.next() != Some(&"]") {
            return Err("Missing closing bracket");
        }
        if tokens.next().is_some() {
            return Err("Trailing tokens");
        }

        // TODO(emilk): support links to fields and enum variants

        let mut is_unreleased = false;
        {
            // Find the target object:
            let mut candidates = vec![];
            for obj in objects.values() {
                if obj.kind.plural_snake_case() == kind && obj.name == type_name {
                    candidates.push(obj);
                }
            }
            if candidates.is_empty() {
                // NOTE: we don't error if the target doesn't exists.
                // Instead we rely on the documentation tools for the different targets,
                // e.g. `cargo doc` and our url link checker.
                // Maybe we could change that though to catch errors earlier.
                re_log::warn_once!("No object found for doclink: [{kind}.{type_name}]");
            } else if candidates.len() > 2 {
                use itertools::Itertools as _;
                re_log::warn_once!(
                    "Multiple objects found for doclink: [{kind}.{type_name}]: {}",
                    candidates.iter().map(|obj| &obj.fqname).format(", ")
                );
            } else if let Some(object) = candidates.first() {
                is_unreleased = object.is_attr_set(crate::ATTR_DOCS_UNRELEASED);
            }
        }

        Ok(match target {
            Target::Cpp => format!("`{kind}::{type_name}`"),
            Target::Rust => {
                // https://doc.rust-lang.org/rustdoc/write-documentation/linking-to-items-by-name.html
                format!("[`{kind}::{type_name}`][crate::{kind}::{type_name}]")
            }
            Target::Python => format!("[`{kind}.{type_name}`][rerun.{kind}.{type_name}]"),
            Target::WebDocsMarkdown => {
                // For instance, https://rerun.io/docs/reference/types/views/spatial2d_view
                // TODO(emilk): relative links would be nicer for the local markdown files
                let type_name_snake_case = re_case::to_snake_case(type_name);
                let query = if is_unreleased {
                    "?speculative-link" // or our link checker will complain
                } else {
                    ""
                };
                format!("[`{kind}.{type_name}`](https://rerun.io/docs/reference/types/{kind}/{type_name_snake_case}{query})")
            }
        })
    }

    fn tokenize(input: &str) -> Vec<&str> {
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
            let objects = Objects::default();

            let input =
                "A vector `[1, 2, 3]` and a doclink [views.Spatial2DView] and a [url](www.rerun.io).";

            assert_eq!(
                translate_doc_line(
                    &objects,
                    input,
                    Target::Cpp
                ),
                "A vector `[1, 2, 3]` and a doclink `views::Spatial2DView` and a [url](www.rerun.io)."
            );

            assert_eq!(
                translate_doc_line(
                    &objects,
                    input,
                    Target::Python
                ),
                "A vector `[1, 2, 3]` and a doclink [`views.Spatial2DView`][rerun.views.Spatial2DView] and a [url](www.rerun.io)."
            );

            assert_eq!(
                translate_doc_line(
                    &objects,
                    input,
                    Target::Rust
                ),
                "A vector `[1, 2, 3]` and a doclink [`views::Spatial2DView`][crate::views::Spatial2DView] and a [url](www.rerun.io)."
            );

            assert_eq!(
                translate_doc_line(
                    &objects,
                    input,
                    Target::WebDocsMarkdown
                ),
                "A vector `[1, 2, 3]` and a doclink [`views.Spatial2DView`](https://rerun.io/docs/reference/types/views/spatial2d_view) and a [url](www.rerun.io)."
            );
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_docs() {
        let objects = Objects::default();

        let docs = Docs::from_lines(
            [
                r" Doclink to [views.Spatial2DView].",
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
            docs.lines_for(&objects, Target::Python),
            vec![
                "Doclink to [`views.Spatial2DView`][rerun.views.Spatial2DView].",
                "",
                "The second line.",
                "",
                "Only for Python.",
                "",
                "The third line.",
            ]
        );

        assert_eq!(
            docs.lines_for(&objects, Target::Cpp),
            vec![
                "Doclink to `views::Spatial2DView`.",
                "",
                "The second line.",
                "",
                "The third line.",
                "",
                "Only for C++.",
            ]
        );

        assert_eq!(
            docs.first_line(&objects, Target::Rust),
            Some("Doclink to [`views::Spatial2DView`][crate::views::Spatial2DView].".to_owned())
        );
    }
}
