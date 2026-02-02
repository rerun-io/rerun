//! Implement a filter for timestamp columns.
//!
//! ## Note on handling of [`arrow::datatypes::DataType::Timestamp`] timezone
//!
//! The arrow documentation specifies that a `Timestamp` datatype with a `None` timezone is a
//! timestamp in an unknown time zone and is thus ambiguous. If the time zone is specified, the
//! physical timestamp is in UTC, the specified time zone is a "hint" about acquisition and,
//! possibly, intended display.
//!
//! Our belief is that the timestamp is actually intended to be UTC in the overwhelming majority of
//! the cases where the time zone is `None`.
//!
//! As a result, this filter is designed such that:
//! - We always consider the physical timestamp to be UTC, even with the time zone is `None`.
//! - We ignore the time zone hint and use instead our global [`TimestampFormat`] for display.

use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::str::FromStr as _;

use arrow::array::{ArrayRef, BooleanArray};
use arrow::datatypes::{DataType, Field, TimeUnit};
use datafusion::common::{Result as DataFusionResult, exec_err};
use datafusion::logical_expr::{Expr, TypeSignature, col, not};
use jiff::{RoundMode, Timestamp, TimestampRound, ToSpan as _};
use re_log_types::TimestampFormat;
use re_types_core::Loggable as _;
use re_types_core::datatypes::TimeInt;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::{DesignTokens, SyntaxHighlighting, UiExt as _};
use strum::VariantArray as _;

use super::{Filter, FilterError, FilterUdf, FilterUiAction, TimestampFormatted, parse_timestamp};

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq, Hash, strum::VariantArray)]
pub enum TimestampOperator {
    #[default]
    Is,
    IsNot,
}

impl Display for TimestampOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Is => f.write_str("is"),
            Self::IsNot => f.write_str("is not"),
        }
    }
}

/// A filter for [`arrow::datatypes::DataType::Timestamp`] columns.
///
/// This represents both the filter itself, and the state of the corresponding UI.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct TimestampFilter {
    /// The kind of temporal filter to use.
    kind: TimestampFilterKind,

    /// Operator to use (is/is not).
    operator: TimestampOperator,

    /// The low bound of the filter (for [`TimestampFilterKind::After`] and
    /// [`TimestampFilterKind::Between`]).
    low_bound_timestamp: EditableTimestamp,

    /// The high bound of the filter (for [`TimestampFilterKind::Before`] and
    /// [`TimestampFilterKind::Between`]).
    high_bound_timestamp: EditableTimestamp,
}

// used for test snapshots, so we make it nice and concise
impl std::fmt::Debug for TimestampFilter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let op_str = match self.operator {
            TimestampOperator::Is => "",
            TimestampOperator::IsNot => "not ",
        };

        let inner = match self.kind {
            TimestampFilterKind::Today => "Today".to_owned(),
            TimestampFilterKind::Yesterday => "Yesterday".to_owned(),
            TimestampFilterKind::Last24Hours => "Last24Hours".to_owned(),
            TimestampFilterKind::ThisWeek => "ThisWeek".to_owned(),
            TimestampFilterKind::LastWeek => "LastWeek".to_owned(),
            TimestampFilterKind::Before => format!("Before {:?}", self.high_bound_timestamp),
            TimestampFilterKind::After => format!("After {:?}", self.low_bound_timestamp),
            TimestampFilterKind::Between => format!(
                "Between {:?} and {:?}",
                self.low_bound_timestamp, self.high_bound_timestamp
            ),
        };

        f.write_str(&format!("TimestampFilter({op_str}{inner})"))
    }
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
            ..Default::default()
        }
    }

    pub fn with_is_not(mut self) -> Self {
        self.operator = TimestampOperator::IsNot;
        self
    }
}

impl SyntaxHighlighting for TimestampFormatted<'_, TimestampFilter> {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_keyword(&self.inner.operator.to_string());
        builder.append_keyword(" ");

        let low_bound = self
            .inner
            .low_bound_timestamp
            .resolved_formatted(self.timestamp_format)
            .unwrap_or_else(|_| "…".to_owned());
        let high_bound = self
            .inner
            .high_bound_timestamp
            .resolved_formatted(self.timestamp_format)
            .unwrap_or_else(|_| "…".to_owned());

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

impl Filter for TimestampFilter {
    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        column_name: &str,
        _popup_just_opened: bool,
    ) -> FilterUiAction {
        ui.horizontal(|ui| {
            ui.label(
                SyntaxHighlightedBuilder::body_default(column_name).into_widget_text(ui.style()),
            );

            egui::ComboBox::new("timestamp_op", "")
                .selected_text(
                    SyntaxHighlightedBuilder::keyword(&self.operator.to_string())
                        .into_widget_text(ui.style()),
                )
                .show_ui(ui, |ui| {
                    for possible_op in TimestampOperator::VARIANTS {
                        if ui
                            .button(
                                SyntaxHighlightedBuilder::keyword(&possible_op.to_string())
                                    .into_widget_text(ui.style()),
                            )
                            .clicked()
                        {
                            self.operator = *possible_op;
                        }
                    }
                });
        });

        // Note on prefilling value for `Before`. Since `Before` generally uses the "high" boundary
        // for computation, it would seem to make sense that the prefilling logic also use the
        // "high" boundary, e.g. when switching from "today" to "before".
        //
        // After testing, we decided to instead use the "low" boundary, because:
        // - visually it makes more sense (it's the "first" one in the UI)
        // - semantically it also kind of makes sense (e.g. if you go from "today" to
        //   "before", you want "before today")

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
            // See note above on `Before`
            TimestampFilterKind::Before => (self.high_bound_timestamp.resolved().ok(), None),
            TimestampFilterKind::After => (self.low_bound_timestamp.resolved().ok(), None),
            TimestampFilterKind::Between => (
                self.low_bound_timestamp.resolved().ok(),
                self.high_bound_timestamp.resolved().ok(),
            ),
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

        let mut action = FilterUiAction::None;

        // handle hitting enter/escape when editing a text field.
        let mut process_text_edit_response = |ui: &egui::Ui, response: &egui::Response| {
            if response.lost_focus() {
                action = action.merge(ui.input(|i| {
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

        ui.add_space(4.0);

        let from_to_header_ui = |ui: &mut egui::Ui, s: &str| {
            ui.label(
                egui::RichText::new(s)
                    .color(ui.tokens().text_default)
                    .size(DesignTokens::list_header_font_size()),
            );

            ui.add_space(-2.0);
        };

        if low_visible {
            from_to_header_ui(
                ui,
                if self.kind == TimestampFilterKind::After {
                    "After:"
                } else {
                    "From:"
                },
            );

            let response = self.low_bound_timestamp.ui(
                ui,
                timestamp_format,
                default_low_string.as_deref(),
                low_is_editable,
            );

            process_text_edit_response(ui, &response);
        }

        if high_visible {
            from_to_header_ui(
                ui,
                if self.kind == TimestampFilterKind::Before {
                    "Before:"
                } else {
                    "To:"
                },
            );

            let response = self.high_bound_timestamp.ui(
                ui,
                timestamp_format,
                // See note above on `Before`
                if self.kind == TimestampFilterKind::Before {
                    default_low_string.as_deref()
                } else {
                    default_high_string.as_deref()
                },
                high_is_editable,
            );

            process_text_edit_response(ui, &response);
        }

        action
    }

    fn on_commit(&mut self) {
        self.low_bound_timestamp.invalidate_timestamp_string();
        self.high_bound_timestamp.invalidate_timestamp_string();
    }

    fn as_filter_expression(&self, field: &Field) -> Result<Expr, FilterError> {
        let udf = ResolvedTimestampFilter::from(self).as_scalar_udf();
        let expr = udf.call(vec![col(field.name().clone())]);

        Ok(match self.operator {
            TimestampOperator::Is => expr,
            TimestampOperator::IsNot => not(expr.clone()).or(expr.is_null()),
        })
    }
}

/// A timestamp that can be entered/edited by the user.
#[derive(Clone)]
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

// used for test snapshots, so we make it nice and concise
impl std::fmt::Debug for EditableTimestamp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.resolved_formatted(TimestampFormat::utc()) {
            Ok(s) => f.write_str(&s),
            Err(err) => f.write_str(&format!("Err({err})")),
        }
    }
}

impl Eq for EditableTimestamp {}

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

impl Hash for EditableTimestamp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.timestamp_string.hash(state);
        self.resolved_timestamp
            .as_ref()
            .map_err(|err| err.to_string())
            .hash(state);
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

        ui.add_space(-4.0);

        match &self.resolved_timestamp {
            Ok(timestamp) => {
                ui.label(format_timestamp(*timestamp, timestamp_format));
            }

            Err(err) => {
                let response = ui.label("YYYY-MM-DD HH:MM:SS");

                if is_editable {
                    response.on_hover_text(err.to_string());
                }
            }
        }

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

    /// Forget about the timestamp string if the resolved timestamp is valid, such that its
    /// normalized next time it is displayed.
    ///
    /// This happens on commit, to cleanup up user input.
    fn invalidate_timestamp_string(&mut self) {
        if self.resolved_timestamp.is_ok() {
            self.timestamp_string = None;
        }
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
///
/// IMPORTANT: this ignores the `TimestampOperator`, because it is applied outside of the UDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

            Self::Last24Hours => {
                // More than one-second accuracy is impossible to get because of how little control
                // the user has over _when_ the filtering is actually executed. So we might as well
                // just round it to the next second to make the UI less weird.
                let next_second = now
                    .round(
                        TimestampRound::new()
                            .smallest(jiff::Unit::Second)
                            .mode(RoundMode::Ceil),
                    )
                    .unwrap_or(now);
                (Some(next_second - 24.hours()), Some(next_second))
            }

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

    fn apply_seconds(&self, seconds: i64) -> bool {
        jiff::Timestamp::from_second(seconds).is_ok_and(|t| self.apply(t))
    }

    fn apply_milliseconds(&self, milli: i64) -> bool {
        jiff::Timestamp::from_millisecond(milli).is_ok_and(|t| self.apply(t))
    }

    fn apply_microseconds(&self, micro: i64) -> bool {
        jiff::Timestamp::from_microsecond(micro).is_ok_and(|t| self.apply(t))
    }

    fn apply_nanoseconds(&self, nano: i64) -> bool {
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

impl FilterUdf for ResolvedTimestampFilter {
    const PRIMITIVE_SIGNATURE: TypeSignature = TypeSignature::Any(1);

    fn name(&self) -> &'static str {
        "timestamp"
    }

    fn is_valid_primitive_input_type(data_type: &DataType) -> bool {
        match data_type {
            _data_type if _data_type == &TimeInt::arrow_datatype() => true,
            DataType::Timestamp(_, _) => true,
            _ => false,
        }
    }

    fn invoke_primitive_array(&self, array: &ArrayRef) -> DataFusionResult<BooleanArray> {
        macro_rules! timestamp_case {
            ($apply_fun:ident, $conv_fun:ident, $op:expr) => {{
                let array = datafusion::common::cast::$conv_fun(array)?;
                let result: BooleanArray =
                    array.iter().map(|x| x.map(|v| $op.$apply_fun(v))).collect();

                Ok(result)
            }};
        }

        match array.data_type() {
            _data_type if _data_type == &TimeInt::arrow_datatype() => {
                timestamp_case!(apply_nanoseconds, as_int64_array, self)
            }

            DataType::Timestamp(TimeUnit::Second, _) => {
                timestamp_case!(apply_seconds, as_timestamp_second_array, self)
            }
            DataType::Timestamp(TimeUnit::Millisecond, _) => {
                timestamp_case!(apply_milliseconds, as_timestamp_millisecond_array, self)
            }
            DataType::Timestamp(TimeUnit::Microsecond, _) => {
                timestamp_case!(apply_microseconds, as_timestamp_microsecond_array, self)
            }
            DataType::Timestamp(TimeUnit::Nanosecond, _) => {
                timestamp_case!(apply_nanoseconds, as_timestamp_nanosecond_array, self)
            }

            _ => {
                exec_err!("Unsupported data type {}", array.data_type())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

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

        // Note: this test doesn't attempt to test close to the limits, because it can become
        // flaky with slow execution time. This is also why we are calling `Timestamp::now()` again
        // for each assert.

        // Should accept timestamps within the last 24 hours
        assert!(filter.apply(Timestamp::now()));
        assert!(filter.apply(Timestamp::now().checked_sub(1.hours()).unwrap()));
        assert!(filter.apply(Timestamp::now().checked_sub(12.hours()).unwrap()));
        assert!(filter.apply(Timestamp::now().checked_sub(23.hours()).unwrap()));

        // Should reject timestamps older than 24 hours
        assert!(!filter.apply(Timestamp::now().checked_sub(25.hours()).unwrap()));
        assert!(!filter.apply(Timestamp::now().checked_sub(48.hours()).unwrap()));

        // Should reject future timestamps
        assert!(!filter.apply(Timestamp::now().checked_add(1.hours()).unwrap()));
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
