//! Utitility functions

/// Returns monotonically increasing time in seconds.
#[inline]
pub fn sec_since_start() -> f64 {
    use web_time::Instant;

    static START_INSTANT: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
    START_INSTANT.elapsed().as_nanos() as f64 / 1e9
}
