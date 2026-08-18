//! Helpers for error handling.

/// Walk the source chain of `error` (starting from `error` itself) and return the
/// first source that can be downcast to `T`.
///
/// The walk is bounded to a small, fixed number of hops to defend against
/// pathological/cyclic chains. Returns `None` if no error in the chain matches `T`
/// within the bound.
pub fn downcast_source<'a, T>(error: &'a (dyn std::error::Error + 'static)) -> Option<&'a T>
where
    T: std::error::Error + 'static,
{
    const MAX_HOPS: usize = 16;

    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    for _ in 0..MAX_HOPS {
        let Some(e) = source else {
            break;
        };
        if let Some(t) = e.downcast_ref::<T>() {
            return Some(t);
        }
        source = e.source();
    }
    None
}

/// The separator used to split error messages into a summary and details.
///
/// If an error message contains this separator, the notification system
/// will display the part before it as the main message and the part after
/// inside a collapsible "Details" section.
///
/// Private on purpose: use [`format_with_details`] to write it, and [`split_details`] to read it.
const DETAILS_SEPARATOR: &str = "\nDetails: ";

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

/// Format an error with its details in a trailing "Details:" section, one detail per line:
/// `{summary}\nDetails: {detail}\n{detail}\n…`.
///
/// Both arguments may already carry details of their own (e.g. an error whose source added some).
/// Those are hoisted out and merged, so that the result has exactly one details section, and a
/// reader only has to look in one place.
///
/// Each detail is normalized to a single trimmed line, so don't put anything in there whose
/// indentation or blank lines carry meaning.
pub fn format_with_details(error: impl Into<String>, details: impl Into<String>) -> String {
    let error = error.into();
    let details = details.into();

    let (summary, mut all_details) = split_details(&error);
    all_details.extend(details_of(&details));

    // Wrapping an error in another one of the same kind repeats details (e.g. the server both
    // errors are about); the reader only needs to see each once.
    let mut seen = std::collections::BTreeSet::new();
    all_details.retain(|detail| seen.insert(*detail));

    if all_details.is_empty() {
        summary.to_owned()
    } else {
        format!("{summary}{DETAILS_SEPARATOR}{}", all_details.join("\n"))
    }
}

/// Like [`format_with_details`], but taking the details one by one.
pub fn format_with_many_details(
    error: impl Into<String>,
    details: impl IntoIterator<Item = impl Into<String>>,
) -> String {
    let details = details
        .into_iter()
        .map(Into::into)
        .collect::<Vec<_>>()
        .join("\n");
    format_with_details(error, details)
}

/// Split a message that may have a trailing "Details:" section into its summary and its details,
/// one entry per detail.
pub fn split_details(message: &str) -> (&str, Vec<&str>) {
    if let Some((summary, details)) = message.split_once(DETAILS_SEPARATOR) {
        (summary.trim_end(), details_of(details).collect())
    } else {
        (message, Vec::new())
    }
}

/// Like [`split_details`], but with the details joined back into a single string.
pub fn split_details_joined(message: &str) -> (&str, Option<String>) {
    let (summary, details) = split_details(message);
    (summary, (!details.is_empty()).then(|| details.join("\n")))
}

/// The individual details in a string of details only (i.e. with the summary already stripped),
/// one per line.
///
/// A nested separator is treated as just another line break, so nested details sections are
/// flattened into this one.
fn details_of(details_only: &str) -> impl Iterator<Item = &str> {
    details_only
        .split(DETAILS_SEPARATOR)
        .flat_map(|section| section.split('\n'))
        .map(str::trim)
        .filter(|detail| !detail.is_empty())
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

    assert_eq!(format_with_details("Error", ""), "Error");

    // Details already in either argument end up in the one and only details section:
    assert_eq!(
        format_with_details("Error\nDetails: from the source", "The fine print"),
        "Error\nDetails: from the source\nThe fine print"
    );
    assert_eq!(
        format_with_details("Error", "trace-id: 42\nDetails: metadata: {}"),
        "Error\nDetails: trace-id: 42\nmetadata: {}"
    );
}

/// Wrapping an error in another one that carries the same detail (e.g. the server both are about)
/// must not list that detail twice.
#[test]
fn test_format_with_details_deduplicates() {
    assert_eq!(
        format_with_details(
            "outer\nDetails: Server: rerun://example.com:443\nouter detail",
            "Server: rerun://example.com:443",
        ),
        "outer\nDetails: Server: rerun://example.com:443\nouter detail"
    );
}

#[test]
fn test_format_with_many_details() {
    assert_eq!(
        format_with_many_details("Error", ["trace-id: 42", "metadata: {}"]),
        "Error\nDetails: trace-id: 42\nmetadata: {}"
    );

    assert_eq!(
        format_with_many_details("Error", Vec::<String>::new()),
        "Error"
    );
}

#[test]
fn test_downcast_source() {
    #[derive(Debug)]
    struct Leaf(&'static str);

    impl std::fmt::Display for Leaf {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    impl std::error::Error for Leaf {}

    #[derive(Debug)]
    struct Wrap(Box<dyn std::error::Error + Send + Sync + 'static>);

    impl std::fmt::Display for Wrap {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "wrap: {}", self.0)
        }
    }

    impl std::error::Error for Wrap {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(self.0.as_ref())
        }
    }

    // Positive: target sits behind a wrapper — walk finds it via `.source()`.
    let wrapped = Wrap(Box::new(Leaf("boom")));
    let found = downcast_source::<Leaf>(&wrapped).expect("Leaf should be recoverable");
    assert_eq!(found.0, "boom");

    // Positive: target IS the top-level error — walk finds it on the first hop.
    let direct = Leaf("direct");
    assert!(downcast_source::<Leaf>(&direct).is_some());

    // Negative: no error in the chain matches `T` — walk terminates with None.
    let only_wrap = Wrap(Box::new(Leaf("inner")));
    assert!(downcast_source::<std::io::Error>(&only_wrap).is_none());
}

#[test]
fn test_split_details() {
    for (in_summary, in_details) in [
        ("just a message", vec![]),
        ("message", vec!["the fine print"]),
        ("message", vec!["first", "second"]),
    ] {
        let combined = format_with_details(in_summary, in_details.join("\n"));
        assert_eq!(split_details(&combined), (in_summary, in_details));
    }

    // A message without any details section is all summary:
    assert_eq!(split_details("just a message"), ("just a message", vec![]));
}
