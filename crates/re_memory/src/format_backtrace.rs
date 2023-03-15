pub fn backtrace_to_string(backtrace: &backtrace::Backtrace) -> String {
    // We need to get a `std::fmt::Formatter`, and there is no easy way to do that, so we do it the hard way:

    struct AnonymizedBacktrace<'a>(&'a backtrace::Backtrace);

    impl<'a> std::fmt::Display for AnonymizedBacktrace<'a> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            format_backtrace(self.0, f)
        }
    }

    AnonymizedBacktrace(backtrace).to_string()
}

fn format_backtrace(
    backtrace: &backtrace::Backtrace,
    fmt: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    let mut print_path = |fmt: &mut std::fmt::Formatter<'_>,
                          path: backtrace::BytesOrWideString<'_>| {
        let path = path.into_path_buf();
        let shortened = shorten_source_file_path(&path);
        std::fmt::Display::fmt(&shortened, fmt)
    };

    let style = if fmt.alternate() {
        backtrace::PrintFmt::Full
    } else {
        backtrace::PrintFmt::Short
    };
    let mut f = backtrace::BacktraceFmt::new(fmt, style, &mut print_path);
    f.add_context()?;
    for frame in backtrace.frames() {
        f.frame().backtrace_frame(frame)?;
    }
    f.finish()?;
    Ok(())
}

/// Anonymize a path to a Rust source file from a callstack.
///
/// Example input:
/// * `/Users/emilk/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-1.24.1/src/runtime/runtime.rs`
/// * `crates/rerun/src/main.rs`
/// * `/rustc/d5a82bbd26e1ad8b7401f6a718a9c57c96905483/library/core/src/ops/function.rs`
fn shorten_source_file_path(path: &std::path::Path) -> String {
    // We must make sure we strip everything sensitive (especially user name).
    // The easiest way is to look for `src` and strip everything up to it.

    use itertools::Itertools as _;
    let components = path.iter().map(|path| path.to_string_lossy()).collect_vec();

    // Look for the last `src`:
    if let Some((src_rev_idx, _)) = components.iter().rev().find_position(|&c| c == "src") {
        let src_idx = components.len() - src_rev_idx - 1;
        // Before `src` comes the name of the crate - let's include that:
        let first_index = src_idx.saturating_sub(1);
        components.iter().skip(first_index).format("/").to_string()
    } else {
        // No `src` directory found - weird!
        path.display().to_string()
    }
}

#[test]
fn test_shorten_path() {
    for (before, after) in [
        ("/Users/emilk/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-1.24.1/src/runtime/runtime.rs", "tokio-1.24.1/src/runtime/runtime.rs"),
        ("crates/rerun/src/main.rs", "rerun/src/main.rs"),
        ("/rustc/d5a82bbd26e1ad8b7401f6a718a9c57c96905483/library/core/src/ops/function.rs", "core/src/ops/function.rs"),
        ("/weird/path/file.rs", "/weird/path/file.rs"),
        ]
        {
        use std::str::FromStr as _;
        let before = std::path::PathBuf::from_str(before).unwrap();
        assert_eq!(shorten_source_file_path(&before), after);
    }
}
