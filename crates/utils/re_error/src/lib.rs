//! Helpers for error handling.

/// The separator used to split error messages into a summary and details.
///
/// If an error message contains this separator, the notification system
/// will display the part before it as the main message and the part after
/// inside a collapsible "Details" section.
///
/// Use [`format_with_details`] to format errors using this convention.
pub const DETAILS_SEPARATOR: &str = "\nDetails:";

/// Format an error, including its chain of sources.
///
/// Always use this when displaying an error, especially `anyhow::Error`.
pub fn format(error: impl AsRef<dyn std::error::Error>) -> String {
    format_ref(error.as_ref())
}

/// Format an error, including its chain of sources.
///
/// Always use this when displaying an error, especially `anyhow::Error`.
pub fn format_ref(error: &dyn std::error::Error) -> String {
    // Use ": " as separator to match anyhow's `format!("{:#}", err)` output
    // See: https://github.com/rerun-io/rerun/issues/8681
    let mut string = error.to_string();
    for source in std::iter::successors(error.source(), |error| error.source()) {
        string.push_str(": ");
        string.push_str(&source.to_string());
    }
    string
}

/// Format an error with details coming after [`DETAILS_SEPARATOR`].
pub fn format_with_details(error: impl Into<String>, details: impl Into<String>) -> String {
    let error = error.into();
    let details = details.into();
    if details.is_empty() {
        error
    } else {
        format!("{error}{DETAILS_SEPARATOR} {details}")
    }
}

/// Split a message that may contain a [`DETAILS_SEPARATOR`] into summary and optional details.
///
/// Returns `(summary, Some(details))` if the separator is present,
/// or `(message, None)` if not.
pub fn split_details(message: &str) -> (&str, Option<&str>) {
    if let Some((summary, details)) = message.split_once(DETAILS_SEPARATOR) {
        let details = details.trim();
        if details.is_empty() {
            (summary, None)
        } else {
            (summary, Some(details))
        }
    } else {
        (message, None)
    }
}

#[test]
fn test_format() {
    let err = anyhow::format_err!("root_cause")
        .context("inner_context")
        .context("outer_context");

    assert_eq!(err.to_string(), "outer_context"); // Oh no, we don't see the root cause!

    // Now we do:
    assert_eq!(format(&err), "outer_context: inner_context: root_cause");
}

#[test]
fn test_format_with_details() {
    assert_eq!(
        format_with_details("Error", "The fine print"),
        "Error\nDetails: The fine print"
    );
}

#[test]
fn test_split_details() {
    for (in_summary, in_details) in [("just a message", ""), ("message", "the fine print")] {
        let combined = format_with_details(in_summary, in_details);
        let (out_summary, out_details) = split_details(&combined);
        assert_eq!(out_summary, in_summary);
        assert_eq!(
            out_details,
            if in_details.is_empty() {
                None
            } else {
                Some(in_details)
            }
        );
    }
}
