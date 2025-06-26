use std::time::Duration;

/// When showing grid-lines representing time.
///
/// Given some spacing (e.g. 10s), return the next spacing (60s).
pub fn next_grid_tick_magnitude_nanos(spacing_nanos: i64) -> i64 {
    if spacing_nanos <= 1_000_000_000 {
        spacing_nanos * 10 // up to 10 second ticks
    } else if spacing_nanos == 10_000_000_000 {
        spacing_nanos * 6 // to the whole minute
    } else if spacing_nanos == 60_000_000_000 {
        spacing_nanos * 10 // to ten minutes
    } else if spacing_nanos == 600_000_000_000 {
        spacing_nanos * 6 // to an hour
    } else if spacing_nanos == 60 * 60 * 1_000_000_000 {
        spacing_nanos * 12 // to 12 h
    } else if spacing_nanos == 12 * 60 * 60 * 1_000_000_000 {
        spacing_nanos * 2 // to a day
    } else {
        spacing_nanos.checked_mul(10).unwrap_or(spacing_nanos) // multiple of ten days
    }
}

/// Formats a timestamp in seconds to a string.
///
/// This is meant for a relatively stable display of timestamps that are typically in minutes
/// where we still care about fractional seconds.
pub fn format_timestamp_secs(timestamp_secs: f64) -> String {
    let n = timestamp_secs as i32;
    let hours = n / (60 * 60);
    let mins = (n / 60) % 60;
    let secs = (n % 60) as f64 + timestamp_secs.fract();

    if hours > 0 {
        format!("{hours:02}:{mins:02}:{secs:02.02}")
    } else {
        format!("{mins:02}:{secs:02.02}")
    }
    // Not showing the minutes at all makes it too unclear what format this timestamp is in.
    // So let's not further strip this down.
}

/// Parses seconds from a string.
///
/// Supports:
/// * fractional seconds
/// * minutes:[fractional seconds]
/// * hours:minutes:[fractional seconds]
pub fn parse_timestamp_secs(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => parts[0].parse::<f64>().ok(),
        2 => {
            let minutes = parts[0].parse::<i32>().ok()?;
            let seconds = parts[1].parse::<f64>().ok()?;
            Some((minutes * 60) as f64 + seconds)
        }
        3 => {
            let hours = parts[0].parse::<i32>().ok()?;
            let minutes = parts[0].parse::<i32>().ok()?;
            let seconds = parts[1].parse::<f64>().ok()?;
            Some(((hours * 60 + minutes) * 60) as f64 + seconds)
        }
        _ => None,
    }
}

/// Formats a duration in a short, readable format, e.g. ("1 hour" or "2 minutes")
///
/// It only shows the largest unit. Shows seconds, minutes, hours, or days.
pub fn format_duration_short(duration: Duration) -> String {
    let seconds = duration.as_secs();

    let format_plural = |n: u64, unit: &str| {
        if n == 1 {
            format!("{} {}", n, unit)
        } else {
            format!("{} {}s", n, unit)
        }
    };

    if seconds < 60 {
        format_plural(seconds, "second")
    } else if seconds < 3600 {
        let minutes = seconds / 60;
        format_plural(minutes, "minute")
    } else if seconds < 86400 {
        let hours = seconds / 3600;
        format_plural(hours, "hour")
    } else {
        let days = seconds / 86400;
        format_plural(days, "day")
    }
}
