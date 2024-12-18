//! Detect and handle signals, panics, and other crashes, making sure to log them and optionally send them off to analytics.

pub mod sigint;

use re_build_info::BuildInfo;

#[cfg(not(target_os = "windows"))]
use parking_lot::Mutex;

// The easiest way to pass this to our signal handler.
#[cfg(not(target_os = "windows"))]
static BUILD_INFO: Mutex<Option<BuildInfo>> = Mutex::new(None);

/// Install handlers for panics and signals (crashes)
/// that prints helpful messages and sends anonymous analytics.
///
/// NOTE: only install these in binaries!
/// * First of all, we don't want to compete with other panic/signal handlers.
/// * Second of all, we don't ever want to include user callstacks in our analytics.
pub fn install_crash_handlers(build_info: BuildInfo) {
    install_panic_hook(build_info);

    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(target_os = "windows"))]
    install_signal_handler(build_info);
}

fn install_panic_hook(_build_info: BuildInfo) {
    let previous_panic_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(
        move |panic_info: &std::panic::PanicHookInfo<'_>| {
            let callstack = callstack_from(&["panicking::panic_fmt\n"]);

            let file_line = panic_info.location().map(|location| {
                let file = anonymize_source_file_path(&std::path::PathBuf::from(location.file()));
                format!("{file}:{}", location.line())
            });

            let msg = panic_info_message(panic_info);

            if let Some(msg) = &msg {
                // Print our own panic message.
                // Our formatting is nicer than `std` since we shorten the file paths (for privacy reasons).
                // This also makes it easier for users to copy-paste the callstack into an issue
                // without having any sensitive data in it.

                let thread = std::thread::current();
                let thread_name = thread
                    .name()
                    .map_or_else(|| format!("{:?}", thread.id()), |name| name.to_owned());

                eprintln!("\nthread '{thread_name}' panicked at '{msg}'");
                if let Some(file_line) = &file_line {
                    eprintln!("{file_line}");
                }
                eprintln!("stack backtrace:\n{callstack}");
            } else {
                // This prints the panic message and callstack:
                (*previous_panic_hook)(panic_info);
            }

            econtext::print_econtext(); // Print additional error context, if any

            eprintln!(
                "\n\
            Troubleshooting Rerun: https://www.rerun.io/docs/getting-started/troubleshooting \n\
            Report bugs: https://github.com/rerun-io/rerun/issues"
            );

            #[cfg(feature = "analytics")]
            {
                if let Ok(analytics) =
                    re_analytics::Analytics::new(std::time::Duration::from_millis(1))
                {
                    analytics.record(re_analytics::event::CrashPanic {
                        build_info: _build_info,
                        callstack,
                        // Don't include panic message, because it can contain sensitive information,
                        // e.g. `panic!("Couldn't read {sensitive_file_path}")`.
                        message: None,
                        file_line,
                    });

                    std::thread::sleep(std::time::Duration::from_secs(1)); // Give analytics time to send the event
                }
            }

            // We compile with `panic = "abort"`, but we don't want to report the same problem twice, so just exit:
            #[allow(clippy::exit)]
            std::process::exit(102);
        },
    ));
}

fn panic_info_message(panic_info: &std::panic::PanicHookInfo<'_>) -> Option<String> {
    // `panic_info.message` is unstable, so this is the recommended way of getting
    // the panic message out. We need both the `&str` and `String` variants.

    #[allow(clippy::manual_map)]
    if let Some(msg) = panic_info.payload().downcast_ref::<&str>() {
        Some((*msg).to_owned())
    } else if let Some(msg) = panic_info.payload().downcast_ref::<String>() {
        Some(msg.clone())
    } else {
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_os = "windows"))]
#[allow(unsafe_code)]
#[allow(clippy::fn_to_numeric_cast_any)]
fn install_signal_handler(build_info: BuildInfo) {
    *BUILD_INFO.lock() = Some(build_info); // Share it with the signal handler

    for signum in [
        libc::SIGABRT,
        libc::SIGBUS,
        libc::SIGFPE,
        libc::SIGILL,
        libc::SIGSEGV,
    ] {
        // SAFETY: we're installing a signal handler.
        unsafe {
            libc::signal(
                signum,
                signal_handler as *const fn(libc::c_int) as libc::size_t,
            );
        }
    }

    unsafe extern "C" fn signal_handler(signal_number: libc::c_int) {
        fn print_problem_and_links(signal_name: &str) {
            write_to_stderr("Rerun caught a signal: ");
            write_to_stderr(signal_name);
            write_to_stderr("\n");
            write_to_stderr(
                "Troubleshooting Rerun: https://www.rerun.io/docs/getting-started/troubleshooting \n",
            );
            write_to_stderr("Report bugs: https://github.com/rerun-io/rerun/issues \n");
            write_to_stderr("\n");
        }

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
        // We take care not to allocate any memory before we generate the call stack.

        write_to_stderr("\n");
        print_problem_and_links(signal_name);

        // Ok, we printed the most important things.
        // Let's do less important things that require memory allocations.
        // Allocating memory can lead to deadlocks if the signal
        // was triggered from the system's memory management functions.

        let callstack = callstack();
        write_to_stderr(&callstack);
        write_to_stderr("\n");

        econtext::print_econtext(); // Print additional error context, if any

        // Let's print the important stuff _again_ so it is visible at the bottom of the users terminal:
        write_to_stderr("\n");
        print_problem_and_links(signal_name);

        // Send analytics - this also sleeps a while to give the analytics time to send the event.
        #[cfg(feature = "analytics")]
        if let Some(build_info) = *BUILD_INFO.lock() {
            send_signal_analytics(build_info, signal_name, callstack);
        }

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
    fn send_signal_analytics(build_info: BuildInfo, signal_name: &str, callstack: String) {
        if let Ok(analytics) = re_analytics::Analytics::new(std::time::Duration::from_millis(1)) {
            analytics.record(re_analytics::event::CrashSignal {
                build_info,
                signal: signal_name.to_owned(),
                callstack,
            });

            std::thread::sleep(std::time::Duration::from_secs(1)); // Give analytics time to send the event
        }
    }

    fn callstack() -> String {
        callstack_from(&["install_signal_handler::signal_handler\n"])
    }
}

/// Get a nicely formatted callstack.
///
/// You can give this function a list of substrings to look for, e.g. names of functions.
/// If any of these substrings matches, anything before that is removed from the callstack.
/// For example:
///
/// ```ignore
/// fn print_callstack() {
///     eprintln!("{}", callstack_from(&["print_callstack"]));
/// }
/// ```
pub fn callstack_from(start_patterns: &[&str]) -> String {
    let backtrace = backtrace::Backtrace::new();
    let stack = backtrace_to_string(&backtrace);

    // Trim it a bit:
    let mut stack = stack.as_str();

    let start_patterns = start_patterns
        .iter()
        .chain(std::iter::once(&"callstack_from"));

    // Trim the top (closest to the panic handler) to cut out some noise:
    for start_pattern in start_patterns {
        if let Some(offset) = stack.find(start_pattern) {
            let prev_newline = stack[..offset].rfind('\n').map_or(0, |newline| newline + 1);
            stack = &stack[prev_newline..];
        }
    }

    // Trim the bottom to cut out code that sets up the callstack:
    let end_patterns = [
        "std::sys_common::backtrace::__rust_begin_short_backtrace",
        // Trim the bottom even more to exclude any user code that potentially used `rerun`
        // as a library to show a viewer. In these cases there may be sensitive user code
        // that called `rerun::run`, and we do not want to include it:
        "run_native_app",
    ];

    for end_pattern in end_patterns {
        if let Some(offset) = stack.find(end_pattern) {
            if let Some(start_of_line) = stack[..offset].rfind('\n') {
                stack = &stack[..start_of_line];
            } else {
                stack = &stack[..offset];
            }
        }
    }

    stack.into()
}

fn backtrace_to_string(backtrace: &backtrace::Backtrace) -> String {
    // We need to get a `std::fmt::Formatter`, and there is no easy way to do that, so we do it the hard way:

    struct AnonymizedBacktrace<'a>(&'a backtrace::Backtrace);

    impl std::fmt::Display for AnonymizedBacktrace<'_> {
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
        let anoymized = anonymize_source_file_path(&path);
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

/// Anonymize a path to a Rust source file from a callstack.
///
/// Example input:
/// * `/Users/emilk/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-1.24.1/src/runtime/runtime.rs`
/// * `crates/rerun/src/main.rs`
/// * `/rustc/d5a82bbd26e1ad8b7401f6a718a9c57c96905483/library/core/src/ops/function.rs`
fn anonymize_source_file_path(path: &std::path::Path) -> String {
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
        assert_eq!(anonymize_source_file_path(&before), after);
    }
}
