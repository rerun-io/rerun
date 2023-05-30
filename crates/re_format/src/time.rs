/// When showing grid-lines representing time.
///
/// Given some spacing (e.g. 10s), return the next spacing (60s).
pub fn next_grid_tick_magnitude_ns(spacing_ns: i64) -> i64 {
    if spacing_ns <= 1_000_000_000 {
        spacing_ns * 10 // up to 10 second ticks
    } else if spacing_ns == 10_000_000_000 {
        spacing_ns * 6 // to the whole minute
    } else if spacing_ns == 60_000_000_000 {
        spacing_ns * 10 // to ten minutes
    } else if spacing_ns == 600_000_000_000 {
        spacing_ns * 6 // to an hour
    } else if spacing_ns == 60 * 60 * 1_000_000_000 {
        spacing_ns * 12 // to 12 h
    } else if spacing_ns == 12 * 60 * 60 * 1_000_000_000 {
        spacing_ns * 2 // to a day
    } else {
        spacing_ns.checked_mul(10).unwrap_or(spacing_ns) // multiple of ten days
    }
}
