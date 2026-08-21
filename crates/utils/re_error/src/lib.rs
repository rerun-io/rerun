//! Helpers for error handling.

mod structured_error;

pub use self::structured_error::{StructuredError, format_with_details};

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
