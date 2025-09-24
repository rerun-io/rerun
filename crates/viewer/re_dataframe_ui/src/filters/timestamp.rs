use std::str::FromStr as _;

use jiff::{Timestamp, ToSpan as _};

use re_log_types::TimestampFormat;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::{SyntaxHighlighting, UiExt as _};

use super::{FilterUiAction, TimestampFormatted, parse_timestamp};

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq)]
enum TimestampFilterKind {
    #[default]
    Today,
    Yesterday,
    Last24Hours,
    ThisWeek,
    LastWeek,
    Before,
    After,
    Between,
}

/// A filter for [`arrow::datatypes::DataType::Timestamp`] columns.
///
/// This represents both the filter itself, and the state of the corresponding UI.
//TODO(ab): a nicer `Debug` implementation would make snapshot tests cleaner.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimestampFilter {
    /// The kind of temporal filter to use.
    kind: TimestampFilterKind,

    /// The low bound of the filter (for [`TimestampFilterKind::After`] and
    /// [`TimestampFilterKind::Between`]).
    low_bound_timestamp: EditableTimestamp,

    /// The high bound of the filter (for [`TimestampFilterKind::Before`] and
    /// [`TimestampFilterKind::Between`]).
    high_bound_timestamp: EditableTimestamp,
}

// constructors
impl TimestampFilter {
    pub fn today() -> Self {
        Self {
            kind: TimestampFilterKind::Today,
            ..Default::default()
        }
    }

    pub fn yesterday() -> Self {
        Self {
            kind: TimestampFilterKind::Yesterday,
            ..Default::default()
        }
    }

    pub fn last_24_hours() -> Self {
        Self {
            kind: TimestampFilterKind::Last24Hours,
            ..Default::default()
        }
    }

    pub fn this_week() -> Self {
        Self {
            kind: TimestampFilterKind::ThisWeek,
            ..Default::default()
        }
    }

    pub fn last_week() -> Self {
        Self {
            kind: TimestampFilterKind::LastWeek,
            ..Default::default()
        }
    }

    pub fn before(high_bound: jiff::Timestamp) -> Self {
        Self {
            kind: TimestampFilterKind::Before,
            high_bound_timestamp: EditableTimestamp::new(high_bound),
            ..Default::default()
        }
    }

    pub fn after(high_bound: jiff::Timestamp) -> Self {
        Self {
            kind: TimestampFilterKind::After,
            low_bound_timestamp: EditableTimestamp::new(high_bound),
            ..Default::default()
        }
    }

    pub fn between(low_bound: jiff::Timestamp, high_bound: jiff::Timestamp) -> Self {
        Self {
            kind: TimestampFilterKind::Between,
            low_bound_timestamp: EditableTimestamp::new(low_bound),
            high_bound_timestamp: EditableTimestamp::new(high_bound),
        }
    }
}

// filtering
impl TimestampFilter {
    pub fn apply_seconds(&self, seconds: i64) -> bool {
        jiff::Timestamp::from_second(seconds)
            .is_ok_and(|t| ResolvedTimestampFilter::from(self).apply(t))
    }

    pub fn apply_milliseconds(&self, milli: i64) -> bool {
        jiff::Timestamp::from_millisecond(milli)
            .is_ok_and(|t| ResolvedTimestampFilter::from(self).apply(t))
    }

    pub fn apply_microseconds(&self, micro: i64) -> bool {
        jiff::Timestamp::from_microsecond(micro)
            .is_ok_and(|t| ResolvedTimestampFilter::from(self).apply(t))
    }

    pub fn apply_nanoseconds(&self, nano: i64) -> bool {
        jiff::Timestamp::from_nanosecond(nano as _)
            .is_ok_and(|t| ResolvedTimestampFilter::from(self).apply(t))
    }
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

        match self.inner.kind {
            TimestampFilterKind::Today => builder.append_keyword("today"),
            TimestampFilterKind::Yesterday => builder.append_keyword("yesterday"),
            TimestampFilterKind::Last24Hours => builder.append_keyword("last 24 hours"),
            TimestampFilterKind::ThisWeek => builder.append_keyword("this week"),
            TimestampFilterKind::LastWeek => builder.append_keyword("last week"),
            TimestampFilterKind::Before => {
                builder.append_keyword("before ");
                builder.append_primitive(&high_bound)
            }
            TimestampFilterKind::After => {
                builder.append_keyword("after ");
                builder.append_primitive(&low_bound)
            }
            TimestampFilterKind::Between => {
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
        let default_timestamp_range = match self.kind {
            TimestampFilterKind::Today => ResolvedTimestampFilter::Today.timestamp_range(),
            TimestampFilterKind::Yesterday => ResolvedTimestampFilter::Yesterday.timestamp_range(),
            TimestampFilterKind::Last24Hours => {
                ResolvedTimestampFilter::Last24Hours.timestamp_range()
            }
            TimestampFilterKind::ThisWeek => ResolvedTimestampFilter::ThisWeek.timestamp_range(),
            TimestampFilterKind::LastWeek => ResolvedTimestampFilter::LastWeek.timestamp_range(),
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

        ui.re_radio_value(&mut self.kind, TimestampFilterKind::Today, "today");
        ui.re_radio_value(&mut self.kind, TimestampFilterKind::Yesterday, "yesterday");
        ui.re_radio_value(
            &mut self.kind,
            TimestampFilterKind::Last24Hours,
            "last 24 hours",
        );
        ui.re_radio_value(&mut self.kind, TimestampFilterKind::ThisWeek, "this week");
        ui.re_radio_value(&mut self.kind, TimestampFilterKind::LastWeek, "last week");
        ui.re_radio_value(&mut self.kind, TimestampFilterKind::Before, "before");
        ui.re_radio_value(&mut self.kind, TimestampFilterKind::After, "after");
        ui.re_radio_value(&mut self.kind, TimestampFilterKind::Between, "between");

        let low_visible = self.kind != TimestampFilterKind::Before;
        let high_visible = self.kind != TimestampFilterKind::After;
        let low_is_editable =
            self.kind == TimestampFilterKind::Between || self.kind == TimestampFilterKind::After;
        let high_is_editable =
            self.kind == TimestampFilterKind::Between || self.kind == TimestampFilterKind::Before;

        // handle hitting enter/escape when editing a text field.
        let mut process_text_edit_response = |ui: &egui::Ui, response: &egui::Response| {
            if response.lost_focus() {
                *action = action.merge(ui.input(|i| {
                    if i.key_pressed(egui::Key::Enter) {
                        FilterUiAction::CommitStateToBlueprint
                    } else if i.key_pressed(egui::Key::Escape) {
                        FilterUiAction::CancelStateEdit
                    } else {
                        FilterUiAction::None
                    }
                }));
            }
        };

        if low_visible {
            ui.label("From:");

            let response = self.low_bound_timestamp.ui(
                ui,
                timestamp_format,
                default_low_string.as_deref(),
                low_is_editable,
            );

            process_text_edit_response(ui, &response);
        }

        if high_visible {
            ui.label("To:");

            let response = self.high_bound_timestamp.ui(
                ui,
                timestamp_format,
                default_high_string.as_deref(),
                high_is_editable,
            );

            process_text_edit_response(ui, &response);
        }
    }
}

/// A timestamp that can be entered/edited by the user.
#[derive(Debug, Clone)]
struct EditableTimestamp {
    /// The timestamp as entered/edited by the user.
    ///
    /// This is an option because seeding this value from a [`jiff::Timestamp`] requires knowledge
    /// of the [`TimestampFormat`], which we might not know at filter creation time. So `None` means
    /// that the user hasn't entered anything yet and the value should be seeded from
    /// `resolved_timestamp` (if available) as soon as the [`TimestampFormat`] is known.
    timestamp_string: Option<String>,

    /// The resolved timestamp (if valid).
    resolved_timestamp: Result<jiff::Timestamp, jiff::Error>,
}

impl Default for EditableTimestamp {
    fn default() -> Self {
        Self {
            timestamp_string: Some(String::new()),
            resolved_timestamp: jiff::Timestamp::from_str(""),
        }
    }
}

impl PartialEq for EditableTimestamp {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            timestamp_string: timestamp,
            resolved_timestamp,
        } = self;

        if !timestamp.eq(&other.timestamp_string) {
            return false;
        }

        match resolved_timestamp {
            Ok(val) => other
                .resolved_timestamp
                .as_ref()
                .is_ok_and(|other_val| val.eq(other_val)),

            // We can't compare the error because it's not PartialEq, but if everything else is
            // equal, the error _should_ be equal as well, and we don't care much if it isn't.
            Err(_) => other.resolved_timestamp.is_err(),
        }
    }
}

impl EditableTimestamp {
    pub fn new(timestamp: jiff::Timestamp) -> Self {
        Self {
            resolved_timestamp: Ok(timestamp),
            timestamp_string: None,
        }
    }

    /// UI for a single timestamp entry.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        auto_fill_value: Option<&str>,
        is_editable: bool,
    ) -> egui::Response {
        self.validate_timestamp_string(timestamp_format);

        if self.timestamp_string.as_ref().is_none_or(String::is_empty)
            && is_editable
            && let Some(auto_fill_value) = auto_fill_value
        {
            self.update_and_resolve_timestamp(auto_fill_value, timestamp_format);
        }

        let response = ui
            .add_enabled_ui(is_editable, |ui| {
                let mut timestamp_string_to_edit = if let Some(auto_fill_value) = auto_fill_value
                    && !is_editable
                {
                    auto_fill_value.to_owned()
                } else {
                    self.timestamp_string.clone().unwrap_or_default()
                };

                let parsed = parse_timestamp(&timestamp_string_to_edit, timestamp_format);
                if parsed.is_err() {
                    ui.style_invalid_field();
                }

                let response = ui.text_edit_singleline(&mut timestamp_string_to_edit);

                if response.changed() && is_editable {
                    self.update_and_resolve_timestamp(timestamp_string_to_edit, timestamp_format);
                }

                response
            })
            .inner;

        match &self.resolved_timestamp {
            Ok(timestamp) => ui.label(format_timestamp(*timestamp, timestamp_format)),

            Err(err) => ui
                .label("YYYY-MM-DD HH:MM:SS")
                .on_hover_text(err.to_string()),
        };

        response
    }

    /// Ensure that the timestamp string is consistent with the current timestamp format.
    ///
    /// It can become inconsistent e.g. when the user changes the timestamp format in the settings.
    /// In this case, we recreate the timestamp string from the resolved timestamp (if any).
    fn validate_timestamp_string(&mut self, timestamp_format: TimestampFormat) {
        if let Ok(resolved) = self.resolved() {
            if let Some(timestamp_string) = self.timestamp_string.as_mut() {
                if !parse_timestamp(timestamp_string, timestamp_format).is_ok_and(|t| t == resolved)
                {
                    *timestamp_string = format_timestamp(resolved, timestamp_format);
                }
            } else {
                self.timestamp_string = Some(format_timestamp(resolved, timestamp_format));
            }
        }
    }

    fn update_and_resolve_timestamp(
        &mut self,
        timestamp_string: impl Into<String>,
        timestamp_format: TimestampFormat,
    ) {
        let timestamp_string = timestamp_string.into();
        self.resolved_timestamp = parse_timestamp(&timestamp_string, timestamp_format);
        self.timestamp_string = Some(timestamp_string);
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

fn format_timestamp(timestamp: Timestamp, timestamp_format: TimestampFormat) -> String {
    re_log_types::Timestamp::from(timestamp).format(timestamp_format)
}

/// Helper to resolve a [`TimestampFilter`] and actually perform the filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedTimestampFilter {
    All,

    Today,
    Yesterday,
    Last24Hours,
    ThisWeek,
    LastWeek,

    /// Accept timestamps that are before the given timestamp.
    Before(jiff::Timestamp),

    /// Accept timestamps that are at or after the given timestamp.
    After(jiff::Timestamp),

    /// Accept timestamps that are at or after the low timestamp, and before the high timestamp.
    Between(jiff::Timestamp, jiff::Timestamp),
}

impl From<&TimestampFilter> for ResolvedTimestampFilter {
    fn from(value: &TimestampFilter) -> Self {
        let low_bound = value.low_bound_timestamp.resolved().ok();
        let high_bound = value.high_bound_timestamp.resolved().ok();

        match value.kind {
            TimestampFilterKind::Today => Self::Today,
            TimestampFilterKind::Yesterday => Self::Yesterday,
            TimestampFilterKind::Last24Hours => Self::Last24Hours,
            TimestampFilterKind::ThisWeek => Self::ThisWeek,
            TimestampFilterKind::LastWeek => Self::LastWeek,
            TimestampFilterKind::Before => {
                if let Some(high_bound) = high_bound {
                    Self::Before(high_bound)
                } else {
                    Self::All
                }
            }
            TimestampFilterKind::After => {
                if let Some(low_bound) = low_bound {
                    Self::After(low_bound)
                } else {
                    Self::All
                }
            }
            TimestampFilterKind::Between => {
                if let (Some(low_bound), Some(high_bound)) = (low_bound, high_bound) {
                    Self::Between(low_bound, high_bound)
                } else {
                    Self::All
                }
            }
        }
    }
}

impl ResolvedTimestampFilter {
    fn timestamp_range(&self) -> (Option<Timestamp>, Option<Timestamp>) {
        let tz = jiff::tz::TimeZone::system();
        let now = Timestamp::now();
        let today = now.to_zoned(tz.clone()).date();

        match self {
            Self::All => (None, None),

            Self::Today => day_range_to_timestamp_range(Some(today), today.tomorrow().ok()),

            Self::Yesterday => day_range_to_timestamp_range(today.yesterday().ok(), Some(today)),

            Self::Last24Hours => (Some(now - 24.hours()), Some(now)),

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

    /// Is the provided timestamp accepted by this filter?
    fn apply(&self, timestamp: jiff::Timestamp) -> bool {
        let (low, high) = self.timestamp_range();

        if let (Some(low), Some(high)) = (low, high)
            && high < low
        {
            return false;
        }

        let mut result = true;

        if let Some(low) = low {
            result &= timestamp >= low;
        }

        if let Some(high) = high {
            result &= timestamp < high;
        }

        result
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
    fn test_last_24_hours_filter() {
        let filter = ResolvedTimestampFilter::Last24Hours;
        let now = Timestamp::now();

        std::thread::sleep(std::time::Duration::from_millis(100));
        // Should accept timestamps within the last 24 hours
        assert!(filter.apply(now));
        assert!(filter.apply(now.checked_sub(1.hours()).unwrap()));
        assert!(filter.apply(now.checked_sub(12.hours()).unwrap()));
        assert!(filter.apply(now.checked_sub(23.hours()).unwrap()));

        // Edge case: just within 24 hours
        let almost_24_hours_ago = now
            .checked_sub(24.hours())
            .unwrap()
            .checked_add(1.seconds())
            .unwrap();
        assert!(filter.apply(almost_24_hours_ago));

        // Should reject timestamps older than 24 hours
        assert!(!filter.apply(now.checked_sub(25.hours()).unwrap()));
        assert!(!filter.apply(now.checked_sub(48.hours()).unwrap()));

        // Should reject future timestamps
        assert!(!filter.apply(now.checked_add(1.seconds()).unwrap()));
        assert!(!filter.apply(now.checked_add(1.hours()).unwrap()));
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
