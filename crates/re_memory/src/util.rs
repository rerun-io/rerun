/// Returns monotonically increasing time in seconds.
#[inline]
pub fn sec_since_start() -> f64 {
    use once_cell::sync::Lazy;
    use web_time::Instant;

    static START_INSTANT: Lazy<Instant> = Lazy::new(Instant::now);
    START_INSTANT.elapsed().as_nanos() as f64 / 1e9
}
