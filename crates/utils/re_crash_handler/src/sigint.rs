use std::sync::atomic::AtomicBool;

// ---

static SIGINT_RECEIVED: AtomicBool = AtomicBool::new(false);

/// Call this to start tracking `SIGINT`s.
///
/// You can then call [`was_sigint_ever_caught`] at any point in time.
#[cfg(not(any(target_os = "windows", target_arch = "wasm32")))]
#[expect(unsafe_code)]
#[expect(clippy::fn_to_numeric_cast_any)]
pub fn track_sigint() {
    static ONCE: std::sync::Once = std::sync::Once::new();

    ONCE.call_once(|| {
        // SAFETY: we're installing a signal handler.
        unsafe {
            libc::signal(
                libc::SIGINT,
                signal_handler as *const fn(libc::c_int) as libc::size_t,
            );
        }

        unsafe extern "C" fn signal_handler(signum: libc::c_int) {
            SIGINT_RECEIVED.store(true, std::sync::atomic::Ordering::Relaxed);

            // SAFETY: we're calling a signal handler.
            unsafe {
                libc::signal(signum, libc::SIG_DFL);
                libc::raise(signum);
            }
        }
    });
}

#[cfg(any(target_os = "windows", target_arch = "wasm32"))]
pub fn track_sigint() {}

/// Returns whether a `SIGINT` was ever caught.
///
/// Need to call [`track_sigint`] at least once first.
pub fn was_sigint_ever_caught() -> bool {
    // If somebody forgot to call this, at least we will only miss the first SIGINT, but
    // SIGINT-spamming will still work.
    track_sigint();

    SIGINT_RECEIVED.load(std::sync::atomic::Ordering::Relaxed)
}
