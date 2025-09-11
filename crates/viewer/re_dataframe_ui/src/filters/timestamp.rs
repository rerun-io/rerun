use jiff::{Timestamp, ToSpan as _};

use re_ui::SyntaxHighlighting;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

/// Timestamp filters.
///
/// The inner
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampFilter {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,

    /// Accept timestamps that are before the given timestamp.
    Before(jiff::Timestamp),

    /// Accept timestamps that are at or after the given timestamp.
    After(jiff::Timestamp),

    /// Accept timestamps that are at or after the low timestamp, and before the high timestamp.
    Between(jiff::Timestamp, jiff::Timestamp),
}

impl SyntaxHighlighting for TimestampFilter {
    //TODO: this stuff should use re_format
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        match self {
            Self::Today => builder.append_keyword("is today"),
            Self::Yesterday => builder.append_keyword("is yesterday"),
            Self::ThisWeek => builder.append_keyword("is this week"),
            Self::LastWeek => builder.append_keyword("is last week"),
            Self::Before(high) => {
                builder.append_keyword("is before ");
                builder.append_primitive(&high.to_string())
            }
            Self::After(low) => {
                builder.append_keyword("is after ");
                builder.append_primitive(&low.to_string())
            }
            Self::Between(low, high) => {
                builder.append_keyword("between ");
                builder.append_primitive(&low.to_string());
                builder.append_keyword(" and ");
                builder.append_primitive(&high.to_string())
            }
        };
    }
}

/// Convert day boundaries into timestamp boundaries.
///
/// Low timestamp is low date at midnight, and high timestamp is high date at midnight. Both using
/// the system timezone.
fn day_range_to_timestamp_range(
    low: Option<jiff::civil::Date>,
    high: Option<jiff::civil::Date>,
) -> (Option<Timestamp>, Option<Timestamp>) {
    let tz = jiff::tz::TimeZone::system();

    (
        low.and_then(|d| d.at(0, 0, 0, 0).to_zoned(tz.clone()).ok())
            .map(|t| t.timestamp()),
        high.and_then(|d| d.at(0, 0, 0, 0).to_zoned(tz).ok())
            .map(|t| t.timestamp()),
    )
}

impl TimestampFilter {
    pub fn apply(&self, timestamp: jiff::Timestamp) -> bool {
        let tz = jiff::tz::TimeZone::system();
        let today = jiff::Timestamp::now().to_zoned(tz.clone()).date();

        let (low, high) = match self {
            Self::Today => day_range_to_timestamp_range(Some(today), today.tomorrow().ok()),

            Self::Yesterday => day_range_to_timestamp_range(today.yesterday().ok(), Some(today)),

            Self::ThisWeek => {
                let days_since_monday = today.weekday().to_monday_zero_offset();
                let week_start = today.checked_sub(days_since_monday.days()).ok();

                day_range_to_timestamp_range(week_start, week_start.map(|d| d + 7.days()))
            }
            Self::LastWeek => {
                let days_since_monday = today.weekday().to_monday_zero_offset();
                let week_start = today.checked_sub(days_since_monday.days()).ok();

                day_range_to_timestamp_range(week_start.map(|d| d - 7.days()), week_start)
            }
            Self::Before(high) => (None, Some(*high)),
            Self::After(low) => (Some(*low), None),
            Self::Between(low, high) => (Some(*low), Some(*high)),
        };

        let mut result = true;

        if let Some(low) = low {
            result &= timestamp >= low;
        }

        if let Some(high) = high {
            result &= timestamp < high;
        }

        result
    }

    pub fn apply_seconds(&self, seconds: i64) -> bool {
        jiff::Timestamp::from_second(seconds).is_ok_and(|t| self.apply(t))
    }

    pub fn apply_milliseconds(&self, milli: i64) -> bool {
        jiff::Timestamp::from_millisecond(milli).is_ok_and(|t| self.apply(t))
    }

    pub fn apply_microseconds(&self, micro: i64) -> bool {
        jiff::Timestamp::from_microsecond(micro).is_ok_and(|t| self.apply(t))
    }

    pub fn apply_nanoseconds(&self, nano: i64) -> bool {
        jiff::Timestamp::from_nanosecond(nano as _).is_ok_and(|t| self.apply(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use jiff::civil::date;

    #[test]
    fn test_before_filter() {
        let cutoff = Timestamp::from_second(1000000).unwrap();
        let filter = TimestampFilter::Before(cutoff);

        // Should accept timestamps before the cutoff
        assert!(filter.apply(Timestamp::from_second(999999).unwrap()));
        assert!(filter.apply(Timestamp::from_second(0).unwrap()));
        assert!(filter.apply(Timestamp::from_second(-1000000).unwrap()));

        // Should reject timestamps at or after the cutoff
        assert!(!filter.apply(cutoff));
        assert!(!filter.apply(Timestamp::from_second(1000001).unwrap()));
        assert!(!filter.apply(Timestamp::from_second(2000000).unwrap()));
    }

    #[test]
    fn test_after_filter() {
        let cutoff = Timestamp::from_second(1000000).unwrap();
        let filter = TimestampFilter::After(cutoff);

        // Should accept timestamps at or after the cutoff
        assert!(filter.apply(cutoff));
        assert!(filter.apply(Timestamp::from_second(1000001).unwrap()));
        assert!(filter.apply(Timestamp::from_second(2000000).unwrap()));

        // Should reject timestamps before the cutoff
        assert!(!filter.apply(Timestamp::from_second(999999).unwrap()));
        assert!(!filter.apply(Timestamp::from_second(0).unwrap()));
        assert!(!filter.apply(Timestamp::from_second(-1000000).unwrap()));
    }

    #[test]
    fn test_between_filter() {
        let low = Timestamp::from_second(1000000).unwrap();
        let high = Timestamp::from_second(2000000).unwrap();
        let filter = TimestampFilter::Between(low, high);

        // Should accept timestamps in the range [low, high)
        assert!(filter.apply(low));
        assert!(filter.apply(Timestamp::from_second(1500000).unwrap()));
        assert!(filter.apply(Timestamp::from_second(1999999).unwrap()));

        // Should reject timestamps outside the range
        assert!(!filter.apply(Timestamp::from_second(999999).unwrap()));
        assert!(!filter.apply(high));
        assert!(!filter.apply(Timestamp::from_second(2000001).unwrap()));
    }

    #[test]
    fn test_today_filter() {
        let filter = TimestampFilter::Today;
        let tz = jiff::tz::TimeZone::system();
        let now = Timestamp::now();
        let today = now.to_zoned(tz.clone()).date();

        // Create timestamps for today at various times
        let today_start = today
            .at(0, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();
        let today_noon = today
            .at(12, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();
        let today_end = today
            .at(23, 59, 59, 999_999_999)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();

        // Today's timestamps should be accepted
        assert!(filter.apply(today_start));
        assert!(filter.apply(today_noon));
        assert!(filter.apply(today_end));
        assert!(filter.apply(now));

        // Yesterday and tomorrow should be rejected
        if let Ok(yesterday) = today.yesterday() {
            let yesterday_noon = yesterday
                .at(12, 0, 0, 0)
                .to_zoned(tz.clone())
                .unwrap()
                .timestamp();
            assert!(!filter.apply(yesterday_noon));
        }

        if let Ok(tomorrow) = today.tomorrow() {
            let tomorrow_start = tomorrow
                .at(0, 0, 0, 0)
                .to_zoned(tz.clone())
                .unwrap()
                .timestamp();
            assert!(!filter.apply(tomorrow_start));
        }
    }

    #[test]
    fn test_yesterday_filter() {
        let filter = TimestampFilter::Yesterday;
        let tz = jiff::tz::TimeZone::system();
        let today = Timestamp::now().to_zoned(tz.clone()).date();

        if let Ok(yesterday) = today.yesterday() {
            // Create timestamps for yesterday
            let yesterday_start = yesterday
                .at(0, 0, 0, 0)
                .to_zoned(tz.clone())
                .unwrap()
                .timestamp();
            let yesterday_noon = yesterday
                .at(12, 0, 0, 0)
                .to_zoned(tz.clone())
                .unwrap()
                .timestamp();
            let yesterday_end = yesterday
                .at(23, 59, 59, 999_999_999)
                .to_zoned(tz.clone())
                .unwrap()
                .timestamp();

            // Yesterday's timestamps should be accepted
            assert!(filter.apply(yesterday_start));
            assert!(filter.apply(yesterday_noon));
            assert!(filter.apply(yesterday_end));

            // Today should be rejected
            let today_start = today
                .at(0, 0, 0, 0)
                .to_zoned(tz.clone())
                .unwrap()
                .timestamp();
            assert!(!filter.apply(today_start));

            // Day before yesterday should be rejected
            if let Ok(day_before) = yesterday.yesterday() {
                let day_before_noon = day_before
                    .at(12, 0, 0, 0)
                    .to_zoned(tz.clone())
                    .unwrap()
                    .timestamp();
                assert!(!filter.apply(day_before_noon));
            }
        }
    }

    #[test]
    fn test_this_week_filter() {
        let filter = TimestampFilter::ThisWeek;
        let tz = jiff::tz::TimeZone::system();
        let now = Timestamp::now();
        let today = now.to_zoned(tz.clone()).date();

        // Find this week's Monday
        let days_since_monday = today.weekday().to_monday_zero_offset();
        let week_start = today.checked_sub(days_since_monday.days()).unwrap();

        // Create timestamps for this week
        let monday_start = week_start
            .at(0, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();
        let wednesday_noon = week_start
            .checked_add(2.days())
            .unwrap()
            .at(12, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();
        let sunday_end = week_start
            .checked_add(6.days())
            .unwrap()
            .at(23, 59, 59, 999_999_999)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();

        // This week's timestamps should be accepted
        assert!(filter.apply(monday_start));
        assert!(filter.apply(wednesday_noon));
        assert!(filter.apply(sunday_end));
        assert!(filter.apply(now));

        // Last week's Sunday should be rejected
        let last_sunday = week_start
            .checked_sub(1.days())
            .unwrap()
            .at(23, 59, 59, 999_999_999)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();
        assert!(!filter.apply(last_sunday));

        // Next week's Monday should be rejected
        let next_monday = week_start
            .checked_add(7.days())
            .unwrap()
            .at(0, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();
        assert!(!filter.apply(next_monday));
    }

    #[test]
    fn test_last_week_filter() {
        let filter = TimestampFilter::LastWeek;
        let tz = jiff::tz::TimeZone::system();
        let today = Timestamp::now().to_zoned(tz.clone()).date();

        // Find this week's Monday and then last week's Monday
        let days_since_monday = today.weekday().to_monday_zero_offset();
        let this_week_start = today.checked_sub(days_since_monday.days()).unwrap();
        let last_week_start = this_week_start.checked_sub(7.days()).unwrap();

        // Create timestamps for last week
        let last_monday_start = last_week_start
            .at(0, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();
        let last_wednesday_noon = last_week_start
            .checked_add(2.days())
            .unwrap()
            .at(12, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();
        let last_sunday_end = last_week_start
            .checked_add(6.days())
            .unwrap()
            .at(23, 59, 59, 999_999_999)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();

        // Last week's timestamps should be accepted
        assert!(filter.apply(last_monday_start));
        assert!(filter.apply(last_wednesday_noon));
        assert!(filter.apply(last_sunday_end));

        // This week's Monday should be rejected
        let this_monday = this_week_start
            .at(0, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();
        assert!(!filter.apply(this_monday));

        // Two weeks ago should be rejected
        let two_weeks_ago = last_week_start
            .checked_sub(1.days())
            .unwrap()
            .at(12, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();
        assert!(!filter.apply(two_weeks_ago));
    }

    #[test]
    fn test_week_filter_on_different_weekdays() {
        // Create a known date (a Wednesday)
        let wednesday = date(2024, 10, 30); // October 30, 2024 is a Wednesday
        let days_since_monday = wednesday.weekday().to_monday_zero_offset();
        assert_eq!(days_since_monday, 2); // Verify it's Wednesday

        // Calculate week boundaries
        let week_start = wednesday.checked_sub(days_since_monday.days()).unwrap();
        let last_week_start = week_start.checked_sub(7.days()).unwrap();

        // Test the calculation logic that TimestampFilter uses
        assert_eq!(week_start, date(2024, 10, 28)); // Monday of that week
        assert_eq!(last_week_start, date(2024, 10, 21)); // Monday of previous week
    }

    #[test]
    fn test_boundary_conditions() {
        let tz = jiff::tz::TimeZone::system();
        let today = Timestamp::now().to_zoned(tz.clone()).date();

        // Test exact midnight boundaries for Today filter
        let filter = TimestampFilter::Today;

        let today_midnight = today
            .at(0, 0, 0, 0)
            .to_zoned(tz.clone())
            .unwrap()
            .timestamp();

        // One nanosecond before midnight today
        let almost_midnight = today_midnight.checked_sub(1.nanoseconds()).unwrap();

        assert!(filter.apply(today_midnight)); // Should include start of today
        assert!(!filter.apply(almost_midnight)); // Should exclude just before today

        if let Ok(tomorrow) = today.tomorrow() {
            let tomorrow_midnight = tomorrow
                .at(0, 0, 0, 0)
                .to_zoned(tz.clone())
                .unwrap()
                .timestamp();

            // One nanosecond before tomorrow's midnight
            let end_of_today = tomorrow_midnight.checked_sub(1.nanoseconds()).unwrap();

            assert!(filter.apply(end_of_today)); // Should include end of today
            assert!(!filter.apply(tomorrow_midnight)); // Should exclude start of tomorrow
        }
    }
}
