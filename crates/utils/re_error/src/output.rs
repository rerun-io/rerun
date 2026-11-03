/// Source of a broken pipe error from [`try_println!`](crate::try_println!),
/// [`try_print!`](crate::try_print!), [`try_eprintln!`](crate::try_eprintln!)
/// or [`try_eprint!`](crate::try_eprint!).
///
/// Find it in an error chain with [`is_output_closed_error`],
/// e.g. to exit with status 0 for `rerun rrd print | head`.
#[derive(Debug)]
struct OutputClosed;

impl std::fmt::Display for OutputClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("output was closed")
    }
}

impl std::error::Error for OutputClosed {}

/// Inner error of the [`std::io::Error`] from [`tag_output_closed`].
///
/// [`std::error::Error::source`] of a [`std::io::Error`] returns the source of its inner error,
/// so `OutputClosed` is in the source chain of the [`std::io::Error`] itself.
/// It stays there when another error wraps the [`std::io::Error`] with `#[error(transparent)]`.
#[derive(Debug)]
struct BrokenPipe {
    source: OutputClosed,
}

impl std::fmt::Display for BrokenPipe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("broken pipe")
    }
}

impl std::error::Error for BrokenPipe {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Gives a broken pipe error the source `OutputClosed`,
/// and returns other errors as they are.
///
/// Used by [`try_println!`](crate::try_println!), [`try_print!`](crate::try_print!),
/// [`try_eprintln!`](crate::try_eprintln!) and [`try_eprint!`](crate::try_eprint!).
pub fn tag_output_closed(err: std::io::Error) -> std::io::Error {
    if err.kind() == std::io::ErrorKind::BrokenPipe {
        std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            BrokenPipe {
                source: OutputClosed,
            },
        )
    } else {
        err
    }
}

/// Whether `err` or one of its sources is a tagged broken pipe error.
///
/// [`try_println!`](crate::try_println!), [`try_print!`](crate::try_print!),
/// [`try_eprintln!`](crate::try_eprintln!) and [`try_eprint!`](crate::try_eprint!)
/// return tagged broken pipe errors.
pub fn is_output_closed_error(err: &(dyn std::error::Error + 'static)) -> bool {
    std::iter::successors(Some(err), |cause| cause.source()).any(|cause| cause.is::<OutputClosed>())
}

/// Like [`println!`], but returns the [`std::io::Error`] instead of panicking.
///
/// A broken pipe error can be found with [`is_output_closed_error`].
#[macro_export]
macro_rules! try_println {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        writeln!(std::io::stdout(), $($arg)*).map_err($crate::tag_output_closed)
    }};
}

/// Like [`print!`], but returns the [`std::io::Error`] instead of panicking.
///
/// A broken pipe error can be found with [`is_output_closed_error`].
#[macro_export]
macro_rules! try_print {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        write!(std::io::stdout(), $($arg)*).map_err($crate::tag_output_closed)
    }};
}

/// Like [`eprintln!`], but returns the [`std::io::Error`] instead of panicking.
///
/// A broken pipe error can be found with [`is_output_closed_error`].
#[macro_export]
macro_rules! try_eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        writeln!(std::io::stderr(), $($arg)*).map_err($crate::tag_output_closed)
    }};
}

/// Like [`eprint!`], but returns the [`std::io::Error`] instead of panicking.
///
/// A broken pipe error can be found with [`is_output_closed_error`].
#[macro_export]
macro_rules! try_eprint {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        write!(std::io::stderr(), $($arg)*).map_err($crate::tag_output_closed)
    }};
}

/// A tagged broken pipe is found on its own, behind `anyhow` context, and behind an error that
/// forwards `source` to the [`std::io::Error`] like `#[error(transparent)]` does.
/// Other errors, including an untagged broken pipe, are not found.
#[test]
fn test_is_output_closed_error() {
    #[derive(Debug)]
    struct Transparent(std::io::Error);

    impl std::fmt::Display for Transparent {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.0.fmt(f)
        }
    }

    impl std::error::Error for Transparent {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.0.source()
        }
    }

    let broken_pipe = || tag_output_closed(std::io::ErrorKind::BrokenPipe.into());

    assert!(is_output_closed_error(&broken_pipe()));
    assert!(is_output_closed_error(
        anyhow::Error::from(broken_pipe())
            .context("writing")
            .as_ref()
    ));
    assert!(is_output_closed_error(&Transparent(broken_pipe())));

    let untagged: std::io::Error = std::io::ErrorKind::BrokenPipe.into();
    assert!(!is_output_closed_error(&untagged));
    assert!(!is_output_closed_error(&tag_output_closed(
        std::io::ErrorKind::NotFound.into()
    )));
}
