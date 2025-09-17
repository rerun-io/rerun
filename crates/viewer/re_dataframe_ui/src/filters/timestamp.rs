use std::str::FromStr as _;

use jiff::{Timestamp, ToSpan as _};

use re_log_types::{TimestampFormat, TimestampFormatKind};
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::{SyntaxHighlighting, UiExt as _};

use super::{FilterUiAction, TimestampFormatted};

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq)]
pub enum TemporalOperator {
    #[default]
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    Before,
    After,
    Between,
}

/// A timestamp that can be entered/edited by the user.
#[derive(Debug, Clone)]
pub struct EditableTimestamp {
    /// The timestamp as entered/edited by the user.
    timestamp: String,

    /// The resolved timestamp (if valid).
    resolved_timestamp: Result<jiff::Timestamp, jiff::Error>,
}

impl Default for EditableTimestamp {
    fn default() -> Self {
        Self {
            timestamp: String::new(),
            resolved_timestamp: jiff::Timestamp::from_str(""),
        }
    }
}

impl PartialEq for EditableTimestamp {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            timestamp,
            resolved_timestamp,
        } = self;

        if !timestamp.eq(&other.timestamp) {
            return false;
        }

        match resolved_timestamp {
            Ok(val) => other
                .resolved_timestamp
                .as_ref()
                .is_ok_and(|other_val| val.eq(other_val)),

            // We can't compare the error because it's not PartialEq, but if everything else is
            // equal, the error should be the same as well.
            Err(_) => other.resolved_timestamp.is_err(),
        }
    }
}

impl EditableTimestamp {
    pub fn new_from_timestamp(
        timestamp: jiff::Timestamp,
        timestamp_format: TimestampFormat,
    ) -> Self {
        let timestamp = re_log_types::Timestamp::from(timestamp).format(timestamp_format);
        Self {
            resolved_timestamp: best_effort_timestamp_parse(&timestamp, timestamp_format),
            timestamp,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        auto_fill_value: Option<&str>,
        is_editable: bool,
    ) -> egui::Response {
        if self.timestamp.is_empty()
            && is_editable
            && let Some(auto_fill_value) = auto_fill_value
        {
            self.timestamp = auto_fill_value.to_owned();
            self.update_resolved_timestamp(timestamp_format);
        }

        let (response, parsed) = ui
            .add_enabled_ui(is_editable, |ui| {
                let mut default_string;
                let timestamp_to_edit = if let Some(auto_fill_value) = auto_fill_value
                    && !is_editable
                {
                    default_string = auto_fill_value.to_owned();
                    &mut default_string
                } else {
                    &mut self.timestamp
                };

                let parsed = best_effort_timestamp_parse(timestamp_to_edit, timestamp_format);

                if parsed.is_err() {
                    ui.style_invalid_field();
                }

                let response = ui.text_edit_singleline(timestamp_to_edit);

                if response.changed() {
                    self.update_resolved_timestamp(timestamp_format);
                }

                (response, parsed)
            })
            .inner;

        match parsed {
            Ok(timestamp) => {
                ui.label(re_log_types::Timestamp::from(timestamp).format(timestamp_format))
            }
            Err(err) => ui
                .label("YYYY-MM-DD HH:MM:SS")
                .on_hover_text(err.to_string()),
        };

        response
    }

    fn update_resolved_timestamp(&mut self, timestamp_format: TimestampFormat) {
        self.resolved_timestamp = best_effort_timestamp_parse(&self.timestamp, timestamp_format);
    }

    pub fn resolved(&self) -> Result<jiff::Timestamp, &jiff::Error> {
        self.resolved_timestamp.as_ref().map(|ts| *ts)
    }

    pub fn resolved_formatted(
        &self,
        timestamp_format: TimestampFormat,
    ) -> Result<String, &jiff::Error> {
        self.resolved_timestamp
            .as_ref()
            .map(|t| re_log_types::Timestamp::from(*t).format(timestamp_format))
    }
}

//TODO: docstrings
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimestampFilter {
    operator: TemporalOperator,
    low_bound_timestamp: EditableTimestamp,
    high_bound_timestamp: EditableTimestamp,
}

impl SyntaxHighlighting for TimestampFormatted<'_, TimestampFilter> {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        let low_bound = self
            .inner
            .low_bound_timestamp
            .resolved_formatted(self.timestamp_format)
            .unwrap_or("…".to_owned());
        let high_bound = self
            .inner
            .high_bound_timestamp
            .resolved_formatted(self.timestamp_format)
            .unwrap_or("…".to_owned());

        match self.inner.operator {
            TemporalOperator::Today => builder.append_keyword("today"),
            TemporalOperator::Yesterday => builder.append_keyword("yesterday"),
            TemporalOperator::ThisWeek => builder.append_keyword("this week"),
            TemporalOperator::LastWeek => builder.append_keyword("last week"),
            TemporalOperator::Before => {
                builder.append_keyword("before ");
                builder.append_primitive(&high_bound)
            }
            TemporalOperator::After => {
                builder.append_keyword("after ");
                builder.append_primitive(&low_bound)
            }
            TemporalOperator::Between => {
                builder.append_keyword("between ");
                builder.append_primitive(&low_bound);
                builder.append_keyword(" and ");
                builder.append_primitive(&high_bound)
            }
        };
    }
}

impl TimestampFilter {
    pub fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        column_name: &str,
        action: &mut FilterUiAction,
        timestamp_format: TimestampFormat,
    ) {
        super::basic_operation_ui(ui, column_name, "is");

        // these are used as default when:
        // - switching from e.g. "today" to "between"
        // - the user didn't previously enter a value
        let default_timestamp_range = match self.operator {
            TemporalOperator::Today => ResolvedTimestampFilter::Today.timestamp_range(),
            TemporalOperator::Yesterday => ResolvedTimestampFilter::Yesterday.timestamp_range(),
            TemporalOperator::ThisWeek => ResolvedTimestampFilter::ThisWeek.timestamp_range(),
            TemporalOperator::LastWeek => ResolvedTimestampFilter::LastWeek.timestamp_range(),
            _ => (None, None),
        };
        let (default_low_string, default_high_string) = (
            default_timestamp_range
                .0
                .map(|t| format_timestamp(t, timestamp_format)),
            default_timestamp_range
                .1
                .map(|t| format_timestamp(t, timestamp_format)),
        );

        ui.re_radio_value(&mut self.operator, TemporalOperator::Today, "today");
        ui.re_radio_value(&mut self.operator, TemporalOperator::Yesterday, "yesterday");
        ui.re_radio_value(&mut self.operator, TemporalOperator::ThisWeek, "this week");
        ui.re_radio_value(&mut self.operator, TemporalOperator::LastWeek, "last week");
        ui.re_radio_value(&mut self.operator, TemporalOperator::Before, "before");
        ui.re_radio_value(&mut self.operator, TemporalOperator::After, "after");
        ui.re_radio_value(&mut self.operator, TemporalOperator::Between, "between");

        let low_visible = self.operator != TemporalOperator::Before;
        let high_visible = self.operator != TemporalOperator::After;
        let low_is_editable =
            self.operator == TemporalOperator::Between || self.operator == TemporalOperator::After;
        let high_is_editable =
            self.operator == TemporalOperator::Between || self.operator == TemporalOperator::Before;

        let mut validated = false;

        if low_visible {
            ui.label("From:");

            let response = self.low_bound_timestamp.ui(
                ui,
                timestamp_format,
                default_low_string.as_deref(),
                low_is_editable,
            );

            if response.lost_focus() && self.low_bound_timestamp.resolved().is_ok() {
                validated = true;
            }
        }

        if high_visible {
            ui.label("To:");

            let response = self.high_bound_timestamp.ui(
                ui,
                timestamp_format,
                default_high_string.as_deref(),
                high_is_editable,
            );

            if response.lost_focus() && self.high_bound_timestamp.resolved().is_ok() {
                validated = true;
            }
        }

        if validated {
            *action = FilterUiAction::CommitStateToBlueprint;
        }
    }

    pub fn resolve(&self) -> ResolvedTimestampFilter {
        let low_bound = self.low_bound_timestamp.resolved().ok();
        let high_bound = self.high_bound_timestamp.resolved().ok();

        match self.operator {
            TemporalOperator::Today => ResolvedTimestampFilter::Today,
            TemporalOperator::Yesterday => ResolvedTimestampFilter::Yesterday,
            TemporalOperator::ThisWeek => ResolvedTimestampFilter::ThisWeek,
            TemporalOperator::LastWeek => ResolvedTimestampFilter::LastWeek,
            TemporalOperator::Before => {
                if let Some(high_bound) = high_bound {
                    ResolvedTimestampFilter::Before(high_bound)
                } else {
                    ResolvedTimestampFilter::All
                }
            }
            TemporalOperator::After => {
                if let Some(low_bound) = low_bound {
                    ResolvedTimestampFilter::After(low_bound)
                } else {
                    ResolvedTimestampFilter::All
                }
            }
            TemporalOperator::Between => {
                if let (Some(low_bound), Some(high_bound)) = (low_bound, high_bound) {
                    ResolvedTimestampFilter::Between(low_bound, high_bound)
                } else {
                    ResolvedTimestampFilter::All
                }
            }
        }
    }
}

fn best_effort_timestamp_parse(
    value: &str,
    timestamp_format: TimestampFormat,
) -> Result<jiff::Timestamp, jiff::Error> {
    // TODO(#11279): ideally we could use `re_log_types::Timestamp::parse` here, but it is currently
    // bugged and parses nano instead of seconds.
    if timestamp_format.kind() == TimestampFormatKind::UnixEpoch {
        if let Ok(seconds) = value.parse::<f64>() {
            return jiff::Timestamp::from_nanosecond((seconds * 1e9).round() as _);
        } else {
            return Err(jiff::Error::from_args(format_args!(
                "could not parse seconds since unix epoch"
            )));
        };
    }

    //TODO: use `re_log_types::Timestamp::parse` here?

    let err = match jiff::Timestamp::from_str(value) {
        Ok(timestamp) => return Ok(timestamp),
        Err(err) => err,
    };

    if let Ok(date_time) = jiff::civil::DateTime::from_str(value) {
        return Ok(date_time.to_zoned(jiff::tz::TimeZone::UTC)?.timestamp());
    }

    if let Ok(date) = jiff::civil::Date::from_str(value) {
        return Ok(date
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)?
            .timestamp());
    }

    if let Ok(time) = jiff::civil::Time::from_str(value) {
        return Ok(jiff::Timestamp::now()
            .to_zoned(jiff::tz::TimeZone::UTC)
            .date()
            .at(
                time.hour(),
                time.minute(),
                time.second(),
                time.subsec_nanosecond(),
            )
            .to_zoned(jiff::tz::TimeZone::UTC)?
            .timestamp());
    }

    Err(err)
}

fn format_timestamp(timestamp: Timestamp, timestamp_format: TimestampFormat) -> String {
    re_log_types::Timestamp::from(timestamp).format(timestamp_format)
}

// NOTE: we have hardcoded `TimestampFormat` here, but it's ok because this conversion is only used
// in tests.
impl From<ResolvedTimestampFilter> for TimestampFilter {
    fn from(value: ResolvedTimestampFilter) -> Self {
        match value {
            ResolvedTimestampFilter::All => Self {
                operator: TemporalOperator::After,
                low_bound_timestamp: Default::default(),
                high_bound_timestamp: Default::default(),
            },

            ResolvedTimestampFilter::Today => Self {
                operator: TemporalOperator::Today,
                low_bound_timestamp: Default::default(),
                high_bound_timestamp: Default::default(),
            },
            ResolvedTimestampFilter::Yesterday => Self {
                operator: TemporalOperator::Yesterday,
                low_bound_timestamp: Default::default(),
                high_bound_timestamp: Default::default(),
            },
            ResolvedTimestampFilter::ThisWeek => Self {
                operator: TemporalOperator::ThisWeek,
                low_bound_timestamp: Default::default(),
                high_bound_timestamp: Default::default(),
            },
            ResolvedTimestampFilter::LastWeek => Self {
                operator: TemporalOperator::LastWeek,
                low_bound_timestamp: Default::default(),
                high_bound_timestamp: Default::default(),
            },
            ResolvedTimestampFilter::Before(high) => Self {
                operator: TemporalOperator::Before,
                low_bound_timestamp: Default::default(),
                high_bound_timestamp: EditableTimestamp::new_from_timestamp(
                    high,
                    TimestampFormat::utc(),
                ),
            },
            ResolvedTimestampFilter::After(low) => Self {
                operator: TemporalOperator::After,
                low_bound_timestamp: EditableTimestamp::new_from_timestamp(
                    low,
                    TimestampFormat::utc(),
                ),
                high_bound_timestamp: Default::default(),
            },
            ResolvedTimestampFilter::Between(low, high) => Self {
                operator: TemporalOperator::Between,
                low_bound_timestamp: EditableTimestamp::new_from_timestamp(
                    low,
                    TimestampFormat::utc(),
                ),
                high_bound_timestamp: EditableTimestamp::new_from_timestamp(
                    high,
                    TimestampFormat::utc(),
                ),
            },
        }
    }
}

/// Resolved timestamp filter used for the actual computation of the filter.
//TODO: make private?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedTimestampFilter {
    All,

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

impl ResolvedTimestampFilter {
    pub fn timestamp_range(&self) -> (Option<Timestamp>, Option<Timestamp>) {
        let tz = jiff::tz::TimeZone::system();
        let today = jiff::Timestamp::now().to_zoned(tz.clone()).date();

        match self {
            Self::All => (None, None),

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
        }
    }

    pub fn apply(&self, timestamp: jiff::Timestamp) -> bool {
        let (low, high) = self.timestamp_range();

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

#[cfg(test)]
mod tests {
    use super::*;

    use jiff::civil::date;

    #[test]
    fn test_before_filter() {
        let cutoff = Timestamp::from_second(1000000).unwrap();
        let filter = ResolvedTimestampFilter::Before(cutoff);

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
        let filter = ResolvedTimestampFilter::After(cutoff);

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
        let filter = ResolvedTimestampFilter::Between(low, high);

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
        let filter = ResolvedTimestampFilter::Today;
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
        let filter = ResolvedTimestampFilter::Yesterday;
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
        let filter = ResolvedTimestampFilter::ThisWeek;
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
        let filter = ResolvedTimestampFilter::LastWeek;
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

        // Test the calculation logic that ResolvedTimestampFilter uses
        assert_eq!(week_start, date(2024, 10, 28)); // Monday of that week
        assert_eq!(last_week_start, date(2024, 10, 21)); // Monday of previous week
    }

    #[test]
    fn test_boundary_conditions() {
        let tz = jiff::tz::TimeZone::system();
        let today = Timestamp::now().to_zoned(tz.clone()).date();

        // Test exact midnight boundaries for Today filter
        let filter = ResolvedTimestampFilter::Today;

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
