//! A parsed Rust source file.

use ra_ap_syntax::{AstNode as _, Edition, SourceFile as AstSourceFile, TextSize};

/// A Rust source file, parsed into a lossless syntax tree.
pub struct SourceFile {
    /// Path relative to the project root, with `/` separators.
    pub path: String,

    /// The full file contents.
    pub text: String,

    /// The root of the syntax tree.
    ///
    /// The parser recovers from errors, so the tree always covers all of `text`.
    pub tree: AstSourceFile,

    /// Byte offset of the start of each line.
    line_starts: Vec<TextSize>,
}

impl SourceFile {
    /// Parse Rust source code. Never fails: syntax errors become error nodes in the tree.
    pub fn parse(path: String, text: String) -> Self {
        let tree = AstSourceFile::parse(&text, Edition::CURRENT).tree();
        let line_starts = std::iter::chain(
            [TextSize::new(0)],
            text.match_indices('\n')
                .filter_map(|(index, _)| u32::try_from(index + 1).ok())
                .map(TextSize::new),
        )
        .collect();
        Self {
            path,
            text,
            tree,
            line_starts,
        }
    }

    /// The 0-based line containing `offset`.
    pub fn line(&self, offset: TextSize) -> usize {
        self.line_starts.partition_point(|&start| start <= offset) - 1
    }

    /// Byte offset of the start of a 0-based line.
    pub fn line_start(&self, line: usize) -> usize {
        self.line_starts
            .get(line)
            .map_or(self.text.len(), |&start| usize::from(start))
    }

    /// The 1-based character column of `offset` on its line.
    pub fn column(&self, offset: TextSize) -> usize {
        let line_start = self.line_start(self.line(offset));
        self.text[line_start..usize::from(offset)].chars().count() + 1
    }

    /// The text of a 0-based line, without the trailing newline.
    pub fn line_text(&self, line: usize) -> &str {
        let start = self.line_start(line);
        let end = self
            .line_starts
            .get(line + 1)
            .map_or(self.text.len(), |&next| usize::from(next) - 1);
        &self.text[start.min(end)..end]
    }

    /// The number of lines, counting a final line without a trailing newline.
    pub fn num_lines(&self) -> usize {
        self.line_starts.len()
    }

    /// The untyped root node of the syntax tree.
    pub fn syntax(&self) -> &ra_ap_syntax::SyntaxNode {
        self.tree.syntax()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_and_column() {
        let file = SourceFile::parse("a.rs".to_owned(), "ab\nå c\n".to_owned());
        assert_eq!(file.line(TextSize::new(0)), 0);
        assert_eq!(file.line(TextSize::new(2)), 0);
        assert_eq!(file.line(TextSize::new(3)), 1);
        assert_eq!(file.column(TextSize::new(3)), 1);
        assert_eq!(
            file.column(TextSize::new(6)),
            3,
            "`å` is two bytes but one column"
        );
        assert_eq!(file.line_text(1), "å c");
    }
}
