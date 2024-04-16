/// A high-level representation of a flatbuffers object's documentation.
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
        let parse_line = |line: &str| {
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
        };

        let lines: Vec<(String, String)> = docs
            .into_iter()
            .flat_map(|doc| doc.into_iter())
            .map(parse_line)
            .collect();

        for (tag, comment) in &lines {
            assert!(
                matches!(tag.as_str(), "" | "example" | "cpp" | "py" | "rs"),
                "Unsupported tag: '\\{tag} {comment}'"
            );
        }

        Self { lines }
    }

    /// Get all doc lines that start with the given tag.
    ///
    /// For instance, pass `"example"` to get all lines that start with `"\example"`.
    pub fn doc_lines_tagged(&self, tag: &str) -> Vec<&str> {
        self.lines_with_tag_matching(|t| t == tag)
    }

    /// Get all doc lines that are untagged.
    pub fn untagged(&self) -> Vec<String> {
        self.lines_with_tag_matching(|t| t.is_empty())
            .iter()
            .map(|&s| s.to_owned())
            .collect()
    }

    /// Get the first line of the documentation untagged.
    pub fn first_line(&self) -> Option<&str> {
        self.lines_with_tag_matching(|t| t.is_empty())
            .first()
            .copied()
    }

    /// Get all doc lines that are untagged, or match the given tag.
    ///
    /// For instance, pass `"py"` to get all lines that are untagged or starta with `"\py"`.
    pub fn doc_lines_for_untagged_and(&self, tag: &str) -> Vec<String> {
        self.lines_with_tag_matching(|t| t.is_empty() || t == tag)
            .iter()
            .map(|&s| s.to_owned())
            .collect()
    }

    pub fn lines_with_tag_matching(&self, include_tag: impl Fn(&str) -> bool) -> Vec<&str> {
        let mut lines: Vec<&str> = self
            .lines
            .iter()
            .filter_map(|(tag, line)| {
                if include_tag(tag) {
                    Some(line.as_str())
                } else {
                    None
                }
            })
            .collect();

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
}
