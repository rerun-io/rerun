/// Install handlers for panics and signals (crashes)
/// that prints helpful messages and sends anonymous analytics.
///
/// NOTE: only install these in binaries!
/// * First of all, we don't want to compete with other panic/signal handlers.
/// * Second of all, we don't ever want to include user callstacks in our analytics.
pub fn install_crash_handlers() {
    install_panic_hook();

    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(target_os = "windows"))]
    install_signal_handler();
}

fn install_panic_hook() {
    let previous_panic_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info: &std::panic::PanicInfo<'_>| {
        // This prints the callstack etc
        (*previous_panic_hook)(panic_info);

        #[cfg(feature = "analytics")]
        {
            if let Ok(analytics) = re_analytics::Analytics::new(std::time::Duration::from_millis(1))
            {
                let callstack = callstack_from("panicking::panic_fmt\n");
                let mut event = re_analytics::Event::append("panic".into())
                    .with_prop("callstack".into(), callstack);
                if let Some(location) = panic_info.location() {
                    event = event.with_prop(
                        "location".into(),
                        format!("{}:{}", location.file(), location.line()),
                    );
                }
                analytics.record(event);

                std::thread::sleep(std::time::Duration::from_secs(1)); // Give analytics time to send the event
            }
        }

        eprintln!(
            "\n\
            Troubleshooting Rerun: https://www.rerun.io/docs/getting-started/troubleshooting"
        );
    }));
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_os = "windows"))]
#[allow(unsafe_code)]
#[allow(clippy::fn_to_numeric_cast_any)]
fn install_signal_handler() {
    // SAFETY: we're installing a signal handler.
    unsafe {
        for signum in [
            libc::SIGABRT,
            libc::SIGBUS,
            libc::SIGFPE,
            libc::SIGILL,
            libc::SIGINT,
            libc::SIGSEGV,
            libc::SIGTERM,
        ] {
            libc::signal(
                signum,
                signal_handler as *const fn(libc::c_int) as libc::size_t,
            );
        }
    }

    unsafe extern "C" fn signal_handler(signal_number: libc::c_int) {
        let signal_name = match signal_number {
            libc::SIGABRT => "SIGABRT",
            libc::SIGBUS => "SIGBUS",
            libc::SIGFPE => "SIGFPE",
            libc::SIGILL => "SIGILL",
            libc::SIGINT => "SIGINT",
            libc::SIGSEGV => "SIGSEGV",
            libc::SIGTERM => "SIGTERM",
            _ => "UNKNOWN SIGNAL",
        };

        // There are very few things that are safe to do in a signal handler,
        // but writing to stderr is one of them.
        // So we first print out what happened to stderr so we're sure that gets out,
        // then we do the unsafe things, like logging the stack trace.
        // We take care not to allocate any memory along the way.

        write_to_stderr("\n");
        write_to_stderr("Rerun caught a signal: ");
        write_to_stderr(signal_name);
        write_to_stderr("\n");
        write_to_stderr(
            "Troubleshooting Rerun: https://www.rerun.io/docs/getting-started/troubleshooting\n\n",
        );

        // Ok, we printed the most important things.
        // Let's do less important things that require memory allocations.
        // Allocating memory can lead to deadlocks if the signal
        // was triggered from the system's memory management functions.

        let callstack = callstack();
        write_to_stderr(&callstack);

        #[cfg(feature = "analytics")]
        send_signal_analytics(signal_name, callstack);

        // Let's print the important stuff _again_ so it is visible at the bottom of the users terminal:
        write_to_stderr("\n");
        write_to_stderr("Rerun caught a signal: ");
        write_to_stderr(signal_name);
        write_to_stderr("\n");
        write_to_stderr(
            "Troubleshooting Rerun: https://www.rerun.io/docs/getting-started/troubleshooting\n\n",
        );

        // We are done!
        // Call the default signal handler (which usually terminates the app):
        // SAFETY: we're calling a signal handler
        unsafe {
            libc::signal(signal_number, libc::SIG_DFL);
            libc::raise(signal_number);
        }
    }

    fn write_to_stderr(text: &str) {
        // SAFETY: writing to stderr is fine, even in a signal handler.
        unsafe {
            libc::write(libc::STDERR_FILENO, text.as_ptr().cast(), text.len());
        }
    }

    #[cfg(feature = "analytics")]
    fn send_signal_analytics(signal_name: &str, callstack: String) {
        if let Ok(analytics) = re_analytics::Analytics::new(std::time::Duration::from_millis(1)) {
            analytics.record(
                re_analytics::Event::append("signal".into())
                    .with_prop("signal".into(), signal_name.to_owned())
                    .with_prop("callstack".into(), callstack),
            );

            std::thread::sleep(std::time::Duration::from_secs(1)); // Give analytics time to send the event
        }
    }

    fn callstack() -> String {
        callstack_from("install_signal_handler::signal_handler\n")
    }
}

fn callstack_from(start_pattern: &str) -> String {
    let backtrace = backtrace::Backtrace::new();
    let stack = backtrace_to_string(&backtrace);

    // Trim it a bit:
    let mut stack = stack.as_str();
    if let Some(start_offset) = stack.find(start_pattern) {
        stack = &stack[start_offset + start_pattern.len()..];
    }
    if let Some(end_offset) = stack.find("std::sys_common::backtrace::__rust_begin_short_backtrace")
    {
        stack = &stack[..end_offset];
    }

    stack.into()
}

fn backtrace_to_string(backtrace: &backtrace::Backtrace) -> String {
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
        let anoymized = anonymize_path(&path);
        std::fmt::Display::fmt(&anoymized, fmt)
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

fn anonymize_path(path: &std::path::Path) -> String {
    // Example input:
    // * `/Users/emilk/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-1.24.1/src/runtime/runtime.rs`
    // * `crates/rerun/src/main.rs`
    // * `/rustc/d5a82bbd26e1ad8b7401f6a718a9c57c96905483/library/core/src/ops/function.rs`

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
        // let's do a safe fallback and only include the last component (the filename)
        components
            .last()
            .map(|filename| filename.to_string())
            .unwrap_or_default()
    }
}

#[test]
fn test_anonymize_path() {
    for (before, after) in [
        ("/Users/emilk/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-1.24.1/src/runtime/runtime.rs", "tokio-1.24.1/src/runtime/runtime.rs"),
        ("crates/rerun/src/main.rs", "rerun/src/main.rs"),
        ("/rustc/d5a82bbd26e1ad8b7401f6a718a9c57c96905483/library/core/src/ops/function.rs", "core/src/ops/function.rs"),
        ("/weird/path/file.rs", "file.rs"),
        ]
        {
        use std::str::FromStr as _;
        let before = std::path::PathBuf::from_str(before).unwrap();
        assert_eq!(anonymize_path(&before), after);
    }
}
