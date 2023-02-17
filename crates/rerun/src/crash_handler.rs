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

        print_callstack();

        // We seem to have managed printing the callstack - great!
        // Then let's print the important stuff _again_ so it is visible at the bottom of the users terminal:

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

    fn print_callstack() {
        let backtrace = backtrace::Backtrace::new();
        let stack = format!("{backtrace:?}");

        // Trim it a bit:
        let mut stack = stack.as_str();
        let start_pattern = "install_signal_handler::signal_handler\n";
        if let Some(start_offset) = stack.find(start_pattern) {
            stack = &stack[start_offset + start_pattern.len()..];
        }
        if let Some(end_offset) =
            stack.find("std::sys_common::backtrace::__rust_begin_short_backtrace")
        {
            stack = &stack[..end_offset];
        }

        write_to_stderr(stack);
    }
}
