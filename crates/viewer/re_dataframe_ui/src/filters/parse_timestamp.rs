use std::str::FromStr as _;

use re_log_types::{TimestampFormat, TimestampFormatKind};

/// Try _very_ hard to parse user input and extrapolate missing data.
///
/// The primary design goal of this function is to successfully parse any partial timestamp, such
/// that as the user types, we never display an error (so long as the timestamp is valid).
///
/// We also try to usefully extrapolate. For example, use today if the date is missing, etc.
//TODO(ab): support min/max boundary, such that we dont suggest inverted "Between" timestamp?
pub fn parse_timestamp(
    value: &str,
    timestamp_format: TimestampFormat,
) -> Result<jiff::Timestamp, jiff::Error> {
    //
    // second since epoch special case
    //

    if timestamp_format.kind() == TimestampFormatKind::SecondsSinceUnixEpoch
        && let Some(timestamp) = re_log_types::Timestamp::parse_with_format(value, timestamp_format)
    {
        return jiff::Timestamp::from_nanosecond(timestamp.nanos_since_epoch() as _);
    }

    //
    // happy paths
    //

    if let Ok(timestamp) = jiff::Timestamp::from_str(value) {
        return Ok(timestamp);
    }
    if let Ok(date_time) = jiff::Zoned::from_str(value) {
        return Ok(date_time.timestamp());
    }

    //
    // try to parse the date and time separately
    //

    let value = value
        .trim()
        .trim_end_matches('-')
        .trim_end_matches(':')
        .trim_end_matches('+');
    let (date_part, time_part) = split_date_time(value);
    let (date, time) = match (date_part, time_part) {
        ("", "") => return Err(jiff::Error::from_args(format_args!("nothing to parse"))),

        // ("", single_part) should never happen, but we handle it anyway
        (single_part, "") | ("", single_part) => {
            if let Ok(time) = parse_time(single_part) {
                (
                    Ok(jiff::Timestamp::now()
                        .to_zoned(timestamp_format.to_jiff_time_zone())
                        .date()),
                    Ok(time),
                )
            } else {
                (parse_date(single_part), jiff::civil::Time::new(0, 0, 0, 0))
            }
        }

        (date_part, time_part) => (parse_date(date_part), parse_time(time_part)),
    };

    match (date, time) {
        (Ok(date), Ok(time)) => Ok(date
            .to_zoned(timestamp_format.to_jiff_time_zone())?
            .date()
            .at(
                time.hour(),
                time.minute(),
                time.second(),
                time.subsec_nanosecond(),
            )
            .to_zoned(timestamp_format.to_jiff_time_zone())?
            .timestamp()),

        (Err(err), _) | (_, Err(err)) => Err(err),
    }
}

fn split_date_time(value: &str) -> (&str, &str) {
    if let Some(result) = value.split_once('T') {
        return result;
    }
    if let Some(result) = value.split_once(' ') {
        return result;
    }

    (value, "")
}

fn parse_date(date_part: &str) -> Result<jiff::civil::Date, jiff::Error> {
    let date_part = date_part.trim_end_matches('-');

    let patterns = ["%Y-%m-%d", "%Y-%m", "%Y"];
    for pattern in patterns {
        if let Ok(date) = jiff::fmt::strtime::parse(pattern, date_part) {
            return jiff::civil::Date::new(
                date.year()
                    // anything before unix epoch cannot be translated into a timestamp
                    .map(|y| y.max(1970))
                    .ok_or_else(|| jiff::Error::from_args(format_args!("missing year")))?,
                date.month().unwrap_or(1),
                date.day().unwrap_or(1),
            );
        }
    }

    Err(jiff::Error::from_args(format_args!("could not parse date")))
}

fn parse_time(time_part: &str) -> Result<jiff::civil::Time, jiff::Error> {
    let patterns = ["%H:%M:%S%.f", "%H:%M:%S", "%H:%M", "%H"];
    for pattern in patterns {
        if let Ok(time) = jiff::fmt::strtime::parse(pattern, time_part) {
            return jiff::civil::Time::new(
                time.hour().unwrap_or(0),
                time.minute().unwrap_or(0),
                time.second().unwrap_or(0),
                time.subsec_nanosecond().unwrap_or(0),
            );
        }
    }

    Err(jiff::Error::from_args(format_args!("could not parse time")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[expect(clippy::too_many_arguments)]
    fn timestamp_from_parts(
        year: i16,
        month: i8,
        day: i8,
        hour: i8,
        minute: i8,
        second: i8,
        nanos: i32,
        tz: jiff::tz::TimeZone,
    ) -> jiff::Timestamp {
        jiff::civil::DateTime::new(year, month, day, hour, minute, second, nanos)
            .unwrap()
            .to_zoned(tz)
            .unwrap()
            .timestamp()
    }

    #[test]
    fn test_unix_epoch_format() {
        let format = TimestampFormat::unix_epoch();

        // Integer seconds
        assert_eq!(
            parse_timestamp("0", format).unwrap(),
            jiff::Timestamp::from_nanosecond(0).unwrap()
        );
        assert_eq!(
            parse_timestamp("1", format).unwrap(),
            jiff::Timestamp::from_nanosecond(1_000_000_000).unwrap()
        );
        assert_eq!(
            parse_timestamp("1609459200", format).unwrap(), // 2021-01-01 00:00:00 UTC
            jiff::Timestamp::from_nanosecond(1_609_459_200_000_000_000).unwrap()
        );

        // Fractional seconds
        assert_eq!(
            parse_timestamp("1.5", format).unwrap(),
            jiff::Timestamp::from_nanosecond(1_500_000_000).unwrap()
        );
        assert_eq!(
            parse_timestamp("0.123456789", format).unwrap(),
            jiff::Timestamp::from_nanosecond(123_456_789).unwrap()
        );

        // Negative values (before epoch)
        assert_eq!(
            parse_timestamp("-1", format).unwrap(),
            jiff::Timestamp::from_nanosecond(-1_000_000_000).unwrap()
        );

        // Invalid input
        assert!(parse_timestamp("not_a_number", format).is_err());
        assert!(parse_timestamp("", format).is_err());
    }

    #[test]
    fn test_full_timestamp_formats() {
        let format = TimestampFormat::utc();

        // RFC3339 with Z
        assert!(parse_timestamp("2025-01-15T12:30:45Z", format).is_ok());
        assert!(parse_timestamp("2025-01-15T12:30:45.123Z", format).is_ok());
        assert!(parse_timestamp("2025-01-15T12:30:45.123456789Z", format).is_ok());

        // RFC3339 with offset
        assert!(parse_timestamp("2025-01-15T12:30:45+00:00", format).is_ok());
        assert!(parse_timestamp("2025-01-15T12:30:45-05:00", format).is_ok());
        assert!(parse_timestamp("2025-01-15T12:30:45+09:30", format).is_ok());

        // Civil datetime (without timezone)
        let result = parse_timestamp("2025-01-15T12:30:45", format).unwrap();
        let expected = timestamp_from_parts(2025, 1, 15, 12, 30, 45, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // With space separator
        let result = parse_timestamp("2025-01-15 12:30:45", format).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_partial_dates() {
        let format = TimestampFormat::utc();

        // Year only
        let result = parse_timestamp("2025", format).unwrap();
        let expected = timestamp_from_parts(2025, 1, 1, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Year-month
        let result = parse_timestamp("2025-03", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 1, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Full date
        let result = parse_timestamp("2025-03-15", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 15, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_partial_times_with_date() {
        let format = TimestampFormat::utc();

        // Hour only
        let result = parse_timestamp("2025-03-15T14", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 15, 14, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Hour:minute
        let result = parse_timestamp("2025-03-15T14:30", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 15, 14, 30, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Hour:minute:second
        let result = parse_timestamp("2025-03-15T14:30:45", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 15, 14, 30, 45, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // With fractional seconds
        let result = parse_timestamp("2025-03-15T14:30:45.123", format).unwrap();
        let expected = timestamp_from_parts(
            2025,
            3,
            15,
            14,
            30,
            45,
            123_000_000,
            jiff::tz::TimeZone::UTC,
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_time_only_uses_today() {
        let format = TimestampFormat::utc();
        let now = jiff::Timestamp::now();
        let today = now.to_zoned(jiff::tz::TimeZone::UTC).date();

        // Time only (no date part)
        let result = parse_timestamp("14:30:45", format).unwrap();
        let result_date = result.to_zoned(jiff::tz::TimeZone::UTC).date();
        assert_eq!(result_date, today);

        // Verify time is correct
        let result_time = result.to_zoned(jiff::tz::TimeZone::UTC).time();
        assert_eq!(result_time.hour(), 14);
        assert_eq!(result_time.minute(), 30);
        assert_eq!(result_time.second(), 45);

        // Hour only
        let result = parse_timestamp("23", format).unwrap();
        let result_date = result.to_zoned(jiff::tz::TimeZone::UTC).date();
        assert_eq!(result_date, today);
        let result_time = result.to_zoned(jiff::tz::TimeZone::UTC).time();
        assert_eq!(result_time.hour(), 23);
        assert_eq!(result_time.minute(), 0);
    }

    #[test]
    fn test_trailing_separators_trimmed() {
        let format = TimestampFormat::utc();

        // Trailing dashes
        let result = parse_timestamp("2025-", format).unwrap();
        let expected = timestamp_from_parts(2025, 1, 1, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        let result = parse_timestamp("2025-03-", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 1, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Trailing colons
        let result = parse_timestamp("2025-03-15T14:", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 15, 14, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        let result = parse_timestamp("2025-03-15T14:30:", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 15, 14, 30, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Multiple trailing separators
        let result = parse_timestamp("2025---", format).unwrap();
        let expected = timestamp_from_parts(2025, 1, 1, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        let result = parse_timestamp("14:30:::", format).unwrap();
        let today = jiff::Timestamp::now()
            .to_zoned(jiff::tz::TimeZone::UTC)
            .date();
        let expected = today
            .at(14, 30, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_year_clamping_to_1970() {
        let format = TimestampFormat::utc();

        // Years before 1970 should be clamped to 1970
        let result = parse_timestamp("1969", format).unwrap();
        let expected = timestamp_from_parts(1970, 1, 1, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        let result = parse_timestamp("1900-12-31", format).unwrap();
        let expected = timestamp_from_parts(1970, 12, 31, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        let result = parse_timestamp("0001-06-15", format).unwrap();
        let expected = timestamp_from_parts(1970, 6, 15, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Year 1970 should not be changed
        let result = parse_timestamp("1970", format).unwrap();
        let expected = timestamp_from_parts(1970, 1, 1, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Years after 1970 should not be changed
        let result = parse_timestamp("1971", format).unwrap();
        let expected = timestamp_from_parts(1971, 1, 1, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_timezone_handling() {
        // UTC format
        let utc_format = TimestampFormat::utc();
        let result = parse_timestamp("2025-01-15T12:00:00", utc_format).unwrap();
        let expected = timestamp_from_parts(2025, 1, 15, 12, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // For local timezone tests, we can only verify the parsing succeeds
        // since the actual result depends on the system timezone
        let local_format = TimestampFormat::local_timezone();
        assert!(parse_timestamp("2025-01-15T12:00:00", local_format).is_ok());

        let local_implicit_format = TimestampFormat::local_timezone_implicit();
        assert!(parse_timestamp("2025-01-15T12:00:00", local_implicit_format).is_ok());
    }

    #[test]
    fn test_whitespace_handling() {
        let format = TimestampFormat::utc();

        // Leading and trailing whitespace
        let result = parse_timestamp("  2025-03-15  ", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 15, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        let result = parse_timestamp("\t2025-03-15T14:30:45\n", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 15, 14, 30, 45, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Whitespace with trailing separators
        let result = parse_timestamp("  2025-  ", format).unwrap();
        let expected = timestamp_from_parts(2025, 1, 1, 0, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_error_cases() {
        let format = TimestampFormat::utc();

        // Empty string
        assert!(parse_timestamp("", format).is_err());
        assert!(parse_timestamp("   ", format).is_err());

        // Just separators
        assert!(parse_timestamp("-", format).is_err());
        assert!(parse_timestamp(":", format).is_err());
        assert!(parse_timestamp("T", format).is_err());
        assert!(parse_timestamp("--::", format).is_err());

        // Invalid formats
        assert!(parse_timestamp("not-a-date", format).is_err());
        assert!(parse_timestamp("12345", format).is_err()); // 5-digit year
        assert!(parse_timestamp("2025-13", format).is_err()); // Invalid month
        assert!(parse_timestamp("2025-00", format).is_err()); // Invalid month
        assert!(parse_timestamp("25:00:00", format).is_err()); // Invalid hour
        assert!(parse_timestamp("12:60:00", format).is_err()); // Invalid minute

        // Invalid second (note that 60 sec is accepted because it can actually happen because of
        // leap seconds).
        assert!(parse_timestamp("12:00:61", format).is_err());
    }

    #[test]
    fn test_mixed_separator_formats() {
        let format = TimestampFormat::utc();

        // Space separator
        let result = parse_timestamp("2025-03-15 14:30:45", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 15, 14, 30, 45, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // T separator (standard ISO)
        let result = parse_timestamp("2025-03-15T14:30:45", format).unwrap();
        assert_eq!(result, expected);

        // Partial with space
        let result = parse_timestamp("2025-03-15 14", format).unwrap();
        let expected = timestamp_from_parts(2025, 3, 15, 14, 0, 0, 0, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Partial with T
        let result = parse_timestamp("2025-03-15T14", format).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_fractional_seconds_precision() {
        let format = TimestampFormat::utc();

        // Milliseconds
        let result = parse_timestamp("2025-01-15T12:00:00.123", format).unwrap();
        let expected =
            timestamp_from_parts(2025, 1, 15, 12, 0, 0, 123_000_000, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Microseconds
        let result = parse_timestamp("2025-01-15T12:00:00.123456", format).unwrap();
        let expected =
            timestamp_from_parts(2025, 1, 15, 12, 0, 0, 123_456_000, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Nanoseconds
        let result = parse_timestamp("2025-01-15T12:00:00.123456789", format).unwrap();
        let expected =
            timestamp_from_parts(2025, 1, 15, 12, 0, 0, 123_456_789, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);

        // Single digit fraction
        let result = parse_timestamp("2025-01-15T12:00:00.5", format).unwrap();
        let expected =
            timestamp_from_parts(2025, 1, 15, 12, 0, 0, 500_000_000, jiff::tz::TimeZone::UTC);
        assert_eq!(result, expected);
    }
}
