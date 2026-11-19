//! Lint violations.

use ra_ap_syntax::TextSize;

/// One lint violation in a source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Name of the rule that fired, e.g. `error_name`.
    pub rule: &'static str,

    /// Where the violation starts.
    ///
    /// A `relint: allow(…)` marker must target the line of this offset.
    pub offset: TextSize,

    /// Human-readable explanation, including how to fix the code.
    pub message: String,

    /// An automatic fix, applied with `--fix`.
    pub fix: Option<Fix>,
}

/// An automatic fix: insert some text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fix {
    /// Where to insert.
    pub offset: TextSize,

    /// What to insert.
    pub insert: &'static str,
}

impl Diagnostic {
    /// A violation of `rule`, starting at `offset`.
    pub fn new(rule: &'static str, offset: TextSize, message: impl Into<String>) -> Self {
        Self {
            rule,
            offset,
            message: message.into(),
            fix: None,
        }
    }

    /// Map a diagnostic in a re-parsed snippet back to the file.
    ///
    /// The snippet is the text at `start` in the file, parsed with `prefix_len` bytes in front of it.
    /// Returns `None` for diagnostics in the prefix itself.
    #[must_use]
    pub fn shifted(mut self, prefix_len: TextSize, start: TextSize) -> Option<Self> {
        self.offset = self.offset.checked_sub(prefix_len)? + start;
        if let Some(fix) = &mut self.fix {
            fix.offset = fix.offset.checked_sub(prefix_len)? + start;
        }
        Some(self)
    }

    /// Fix the violation by inserting `insert` at `offset`.
    #[must_use]
    pub fn with_fix(mut self, offset: TextSize, insert: &'static str) -> Self {
        self.fix = Some(Fix { offset, insert });
        self
    }
}
