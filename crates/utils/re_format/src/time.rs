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

/// Formats a relative timestamp (e.g. those in a video) in seconds to a string.
///
/// This is meant for a relatively stable display of timestamps that are typically in minutes
/// where we still care about fractional seconds.
pub fn format_relative_timestamp_secs(timestamp_secs: f64) -> String {
    let n = timestamp_secs as i32;
    let hours = n / (60 * 60);
    let mins = (n / 60) % 60;
    let secs = (n % 60) as f64 + timestamp_secs.fract();

    if hours > 0 {
        format!("{hours:02}:{mins:02}:{secs:05.2}")
    } else {
        format!("{mins:02}:{secs:05.2}")
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
pub fn parse_relative_timestamp_secs(s: &str) -> Option<f64> {
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

#[test]
fn test_format_timestamp_secs() {
    assert_eq!(format_relative_timestamp_secs(3.4), "00:03.40");
    assert_eq!(format_relative_timestamp_secs(3.53), "00:03.53");
    assert_eq!(format_relative_timestamp_secs(63.53), "01:03.53");
}
