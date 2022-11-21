/// Pretty format bytes, e.g.
///
/// ```
/// assert_eq!(format_bytes(123), "123 B");
/// assert_eq!(format_bytes(12_345), "12 kB");
/// assert_eq!(format_bytes(1_234_567), "1.2 MB");
/// assert_eq!(format_bytes(123_456_789), "123 GB");
/// ```
pub fn format_bytes(number_of_bytes: f64) -> String {
    if number_of_bytes < 0.0 {
        return format!("-{}", format_bytes(-number_of_bytes));
    }

    if number_of_bytes < 1000.0 {
        format!("{:.0} B", number_of_bytes)
    } else if number_of_bytes < 1_000_000.0 {
        let decimals = (number_of_bytes < 10_000.0) as usize;
        format!("{:.*} kB", decimals, number_of_bytes / 1_000.0)
    } else if number_of_bytes < 1_000_000_000.0 {
        let decimals = (number_of_bytes < 10_000_000.0) as usize;
        format!("{:.*} MB", decimals, number_of_bytes / 1_000_000.0)
    } else {
        let decimals = (number_of_bytes < 10_000_000_000.0) as usize;
        format!("{:.*} GB", decimals, number_of_bytes / 1_000_000_000.0)
    }
}

/// Returns monotonically increasing time in seconds.
#[inline]
pub fn sec_since_start() -> f64 {
    use instant::Instant;
    use once_cell::sync::Lazy;

    static START_INSTANT: Lazy<Instant> = Lazy::new(Instant::now);
    START_INSTANT.elapsed().as_nanos() as f64 / 1e9
}
