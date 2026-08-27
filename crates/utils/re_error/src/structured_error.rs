//! An error message split into a summary and a list of details.

/// The prefix that marks a line of an error message as a detail rather than part of the summary.
///
/// The notification system shows the summary as the main message, and the details inside a
/// collapsible "Details" section.
///
/// Private on purpose: use [`StructuredError`] to write it and to read it.
const DETAIL_PREFIX: &str = "- ";

/// [`DETAIL_PREFIX`] at the start of a new line: what separates one detail from the next.
const DETAIL_SEPARATOR: &str = "\n- ";

/// An error message split into a short summary and a list of details.
///
/// This is the in-memory form of the `{summary}\n- {detail}\n- {detail}\n…` wire format that errors
/// are passed around as (over gRPC, through `thiserror`, into the notification system, …).
/// Use [`StructuredError::parse`] to parse such a string, and [`std::fmt::Display`] to write one.
///
/// Each detail is trimmed and non-empty, but may span several lines: an unmarked line after a
/// marked one continues the previous detail, like a lazy continuation line in a markdown list.
/// Details are deduplicated: wrapping an error in another one of the same kind tends to repeat
/// details (e.g. the server both errors are about), and the reader only needs to see each once.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StructuredError {
    /// The main message, shown on its own.
    pub summary: String,

    /// Extra context, shown in a collapsible "Details" section.
    ///
    /// The methods here keep these trimmed, non-empty, and deduplicated; keep it that way if you
    /// edit the field directly.
    pub details: Vec<String>,
}

impl StructuredError {
    /// An error with the whole (unparsed) string as the summary, and no details.
    ///
    /// The summary must not contain any line starting with `"- "`, or it will read back as
    /// details; use [`Self::parse`] for strings that may already carry details.
    pub fn from_summary(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            details: Vec::new(),
        }
    }

    /// Parse a message whose trailing lines may be details, i.e. start with `"- "`.
    ///
    /// Infallible: a message without any such line is all summary.
    ///
    /// The details section starts at the first line that starts with `"- "`. After that, each
    /// such line starts a new detail, and an unmarked line continues the previous one (like a
    /// lazy continuation line in a markdown list). So a summary must not contain any line starting
    /// with `"- "`, or the rest of it is read as details.
    pub fn parse(message: impl AsRef<str>) -> Self {
        let message = message.as_ref().trim();

        // If the message is details from its very first line, with no summary:
        if let Some(details) = message.strip_prefix(DETAIL_PREFIX) {
            return Self::from_summary("").with_detail(details);
        }

        match message.split_once(DETAIL_SEPARATOR) {
            Some((summary, details)) => Self::from_summary(summary.trim_end()).with_detail(details),
            None => Self::from_summary(message),
        }
    }

    /// Add one detail, which may itself be several lines and/or carry its own details section.
    #[inline]
    pub fn with_detail(mut self, detail: impl AsRef<str>) -> Self {
        self.add_detail(detail);
        self
    }

    /// Add several details, each of which may itself be several lines.
    #[inline]
    pub fn with_details(mut self, details: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        self.add_details(details);
        self
    }

    /// Add one detail, which may itself be several lines and/or carry its own details section.
    pub fn add_detail(&mut self, detail: impl AsRef<str>) {
        let detail = detail.as_ref().trim();
        let detail = detail.strip_prefix(DETAIL_PREFIX).unwrap_or(detail);
        for part in detail.split(DETAIL_SEPARATOR) {
            let part = part.trim();
            if !part.is_empty() && !self.details.iter().any(|seen| seen == part) {
                self.details.push(part.to_owned());
            }
        }
    }

    /// Add several details, each of which may itself be several lines.
    pub fn add_details(&mut self, details: impl IntoIterator<Item = impl AsRef<str>>) {
        for detail in details {
            self.add_detail(detail);
        }
    }

    /// Concatenate two errors: the summaries are joined by `": "` (like a source chain), and the
    /// details of both end up in the one and only details section, deduplicated.
    ///
    /// Also available as [`std::ops::Add`]: `outer + inner`.
    pub fn concat(mut self, inner: impl Into<Self>) -> Self {
        let Self { summary, details } = inner.into();

        if self.summary.is_empty() {
            self.summary = summary;
        } else if !summary.is_empty() {
            // Use ": " as separator to match anyhow's `format!("{:#}", err)` output
            self.summary.push_str(": ");
            self.summary.push_str(&summary);
        }

        self.add_details(details);
        self
    }

    /// The details as a single string, one `- detail` per line, or `None` if there are none.
    pub fn details_joined(&self) -> Option<String> {
        (!self.details.is_empty()).then(|| {
            self.details
                .iter()
                .map(|detail| format!("{DETAIL_PREFIX}{detail}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
    }
}

impl<Rhs: Into<Self>> std::ops::Add<Rhs> for StructuredError {
    type Output = Self;

    fn add(self, inner: Rhs) -> Self {
        self.concat(inner)
    }
}

impl std::fmt::Display for StructuredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { summary, details } = self;
        f.write_str(summary)?;
        for (i, detail) in details.iter().enumerate() {
            if !summary.is_empty() || 0 < i {
                f.write_str("\n")?;
            }
            write!(f, "{DETAIL_PREFIX}{detail}")?;
        }
        Ok(())
    }
}

impl std::str::FromStr for StructuredError {
    type Err = std::convert::Infallible;

    fn from_str(message: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(message))
    }
}

impl From<&str> for StructuredError {
    fn from(message: &str) -> Self {
        Self::parse(message)
    }
}

impl From<String> for StructuredError {
    fn from(message: String) -> Self {
        Self::parse(message)
    }
}

impl From<StructuredError> for String {
    fn from(error: StructuredError) -> Self {
        error.to_string()
    }
}

/// Format an error with its details on trailing lines, one detail per line:
/// `{summary}\n- {detail}\n- {detail}\n…`.
///
/// Both arguments may already carry details of their own (e.g. an error whose source added some).
/// Those are hoisted out and merged, so that the result has exactly one details section, and a
/// reader only has to look in one place.
///
/// Details are trimmed and deduplicated, so don't put anything in there whose surrounding
/// whitespace carries meaning.
pub fn format_with_details(error: impl AsRef<str>, details: impl AsRef<str>) -> String {
    StructuredError::parse(error)
        .with_detail(details)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_with_details() {
        assert_eq!(
            format_with_details("Error", "The fine print"),
            "Error\n\
             - The fine print"
        );

        assert_eq!(format_with_details("Error", ""), "Error");

        // Details already in either argument end up in the one and only details section:
        assert_eq!(
            format_with_details("Error\n- from the source", "The fine print"),
            "Error\n\
             - from the source\n\
             - The fine print"
        );
        assert_eq!(
            format_with_details("Error", "trace-id: 42\n- metadata: {}"),
            "Error\n\
             - trace-id: 42\n\
             - metadata: {}"
        );
    }

    /// Wrapping an error in another one that carries the same detail (e.g. the server both are
    /// about) must not list that detail twice.
    #[test]
    fn test_format_with_details_deduplicates() {
        assert_eq!(
            format_with_details(
                "outer\n- Server: rerun://example.com:443\n- outer detail",
                "Server: rerun://example.com:443",
            ),
            "outer\n\
             - Server: rerun://example.com:443\n\
             - outer detail"
        );
    }

    #[test]
    fn test_display() {
        assert_eq!(
            StructuredError::parse("Error")
                .with_details(["trace-id: 42", "metadata: {}"])
                .to_string(),
            "Error\n\
             - trace-id: 42\n\
             - metadata: {}"
        );

        assert_eq!(StructuredError::parse("Error").to_string(), "Error");
    }

    #[test]
    fn test_parse() {
        for (in_summary, in_details) in [
            ("just a message", vec![]),
            ("message", vec!["the fine print"]),
            ("message", vec!["first", "second"]),
        ] {
            let combined = StructuredError::parse(in_summary)
                .with_details(&in_details)
                .to_string();
            let error = StructuredError::parse(&combined);
            assert_eq!(error.summary, in_summary);
            assert_eq!(error.details, in_details);
        }

        // A message without any details is all summary, newlines and all:
        let error = StructuredError::parse("just a message\nspanning two lines");
        assert_eq!(error.summary, "just a message\nspanning two lines");
        assert!(error.details.is_empty());
    }

    #[test]
    fn test_round_trip() {
        for message in [
            "just a message",
            "message\n- the fine print",
            "message\n- first\n- second",
            "- a detail without a summary",
        ] {
            let error = StructuredError::parse(message);
            assert_eq!(error.to_string(), message);
            assert_eq!(StructuredError::parse(error.to_string()), error);
        }

        // An error without a summary also round-trips:
        let error = StructuredError::from_summary("").with_detail("the fine print");
        assert_eq!(StructuredError::parse(error.to_string()), error);
    }

    #[test]
    fn test_parse_corner_cases() {
        // Blank details and stray whitespace are dropped, not turned into empty lines:
        let error = StructuredError::parse("message  \n-   first  \n\n  \n- second\n");
        assert_eq!(error.summary, "message");
        assert_eq!(error.details, ["first", "second"]);

        // An unmarked line after a detail continues that detail (markdown lazy continuation):
        let error = StructuredError::parse("message\n- first\nstill first");
        assert_eq!(error.summary, "message");
        assert_eq!(error.details, ["first\nstill first"]);

        // …which makes it distinct from a marked line:
        let error = StructuredError::parse("message\n- first\n- second");
        assert_eq!(error.details, ["first", "second"]);

        // A dash that doesn't start a line is just a dash:
        let error = StructuredError::parse("a - b");
        assert_eq!(error.summary, "a - b");
        assert!(error.details.is_empty());

        // …and neither is a bare dash without the space:
        let error = StructuredError::parse("message\n-tick");
        assert_eq!(error.summary, "message\n-tick");
        assert!(error.details.is_empty());

        // A message that is a detail from its very first line has no summary:
        let error = StructuredError::parse("- a detail without a summary");
        assert_eq!(error.summary, "");
        assert_eq!(error.details, ["a detail without a summary"]);

        // A detail that is already marked is not marked twice:
        let error = StructuredError::parse("message").with_detail("- already marked");
        assert_eq!(error.to_string(), "message\n- already marked");
    }

    #[test]
    fn test_details_are_deduplicated() {
        let error = StructuredError::parse("message")
            .with_detail("same")
            .with_details(["same", "other", "same"]);
        assert_eq!(error.details, ["same", "other"]);
    }

    fn empty_error() -> StructuredError {
        StructuredError::parse("")
    }

    #[test]
    fn test_concat() {
        let outer =
            StructuredError::parse("outer").with_details(["server: example.com", "outer only"]);
        let inner = StructuredError::parse("inner\n- server: example.com\n- inner only");

        let combined = outer.clone() + inner;
        assert_eq!(combined.summary, "outer: inner");
        assert_eq!(
            combined.details,
            ["server: example.com", "outer only", "inner only"]
        );
        assert_eq!(
            combined.to_string(),
            "outer: inner\n\
             - server: example.com\n\
             - outer only\n\
             - inner only"
        );

        // An empty summary on either side must not leave a dangling ": ":
        assert_eq!((empty_error() + outer.clone()).summary, "outer");
        assert_eq!((outer.concat(empty_error())).summary, "outer");

        // The right-hand side can be anything that parses into an error:
        assert_eq!(
            (StructuredError::parse("outer") + "inner\n- the fine print").to_string(),
            "outer: inner\n\
             - the fine print"
        );
    }

    #[test]
    fn test_details_joined() {
        assert_eq!(StructuredError::parse("message").details_joined(), None);
        assert_eq!(
            StructuredError::parse("message")
                .with_details(["a", "b"])
                .details_joined(),
            Some("- a\n- b".to_owned())
        );
    }
}
