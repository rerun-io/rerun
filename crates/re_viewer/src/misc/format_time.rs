/// Useful when showing dates/times on a timeline
/// and you want it compact.
///
/// Shows dates when zoomed out, shows times when zoomed in,
/// shows relative millisecond when really zoomed in.
pub fn format_time_compact(time: re_log_types::Time) -> String {
    let ns = time.nanos_since_epoch();
    let relative_ns = ns % 1_000_000_000;
    let is_whole_second = relative_ns == 0;
    if is_whole_second {
        if let Some(datetime) = time.to_datetime() {
            let is_whole_minute = ns % 60_000_000_000 == 0;
            let time_format = if time.is_exactly_midnight() {
                "[year]-[month]-[day]Z"
            } else if is_whole_minute {
                "[hour]:[minute]Z"
            } else {
                "[hour]:[minute]:[second]Z"
            };
            let parsed_format = time::format_description::parse(time_format).unwrap();
            return datetime.format(&parsed_format).unwrap();
        }

        re_log_types::Duration::from_nanos(ns).to_string()
    } else {
        // We are in the sub-second resolution.
        // Showing the full time (HH:MM:SS.XXX or 3h 2m 6s …) becomes too long,
        // so instead we switch to showing the time as milliseconds since the last whole second:
        let ms = relative_ns as f64 * 1e-6;
        if relative_ns % 1_000_000 == 0 {
            format!("{ms:+.0} ms")
        } else if relative_ns % 100_000 == 0 {
            format!("{ms:+.1} ms")
        } else if relative_ns % 10_000 == 0 {
            format!("{ms:+.2} ms")
        } else if relative_ns % 1_000 == 0 {
            format!("{ms:+.3} ms")
        } else if relative_ns % 100 == 0 {
            format!("{ms:+.4} ms")
        } else if relative_ns % 10 == 0 {
            format!("{ms:+.5} ms")
        } else {
            format!("{ms:+.6} ms")
        }
    }
}

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
