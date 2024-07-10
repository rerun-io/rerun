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

#[cfg(test)]
mod tests {
    use crate::Docs;

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
