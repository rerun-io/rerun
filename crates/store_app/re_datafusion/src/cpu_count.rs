//! Shared logic for sizing CPU parallelism from the `RERUN_SDK_NUM_CPUS` environment variable.

use std::sync::LazyLock;

/// The machine's available parallelism, or `2` if it can't be determined.
///
/// The fallback is `2` rather than `1` so a pool sized from this keeps some concurrency
/// even on the rare platform where the core count can't be detected.
///
/// Computed once and cached for the lifetime of the process.
pub fn available_cpus() -> usize {
    static AVAILABLE_CPUS: LazyLock<usize> =
        LazyLock::new(|| std::thread::available_parallelism().map_or(2, |n| n.get()));
    *AVAILABLE_CPUS
}

/// The value of the `RERUN_SDK_NUM_CPUS` environment variable, clamped to `[1, available_cpus()]`.
///
/// Returns `None` if the variable is unset, leaving the caller free to pick its own default.
/// If the variable is set but unparsable, this `warn_once!`s and returns `None`, so a typo
/// degrades to the default rather than silently pinning some surprising value.
///
/// A fractional value is truncated; clamping to `[1, available_cpus()]` guards against
/// infinity / huge values and ensures we never exceed the machine's actual core count.
///
/// Read once and cached for the lifetime of the process (so changing the variable at
/// runtime has no effect).
pub fn rerun_sdk_num_cpus() -> Option<usize> {
    static NUM_CPUS: LazyLock<Option<usize>> = LazyLock::new(|| {
        let raw = std::env::var("RERUN_SDK_NUM_CPUS").ok()?;
        if let Ok(f) = raw.trim().parse::<f64>() {
            Some((f as usize).clamp(1, available_cpus()))
        } else {
            re_log::warn_once!("Ignoring unparsable RERUN_SDK_NUM_CPUS={raw:?}");
            None
        }
    });
    *NUM_CPUS
}
