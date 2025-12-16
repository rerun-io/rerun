use re_log_types::external::re_types_core::datatypes::{TimeInt, TimeRange, TimeRangeBoundary};
use re_log_types::{AbsoluteTimeRange, TimeType, TimestampFormat};

use crate::list_item::{self, LabelContent};
use crate::{TimeDragValue, UiExt as _};

/// A time range that can be relative to the time cursor.
pub struct RelativeTimeRange<'a> {
    pub time_drag_value: &'a TimeDragValue,
    pub value: &'a mut TimeRange,
    pub resolved_range: AbsoluteTimeRange,
    pub time_type: TimeType,
    pub timestamp_format: TimestampFormat,
    pub current_time: TimeInt,
}

pub fn relative_time_range_boundary_label_text(
    boundary: TimeRangeBoundary,
    time_type: TimeType,
    low_bound: bool,
) -> &'static str {
    match boundary {
        TimeRangeBoundary::CursorRelative(_) => match time_type {
            TimeType::DurationNs | TimeType::TimestampNs => "current time with offset",
            TimeType::Sequence => "current frame with offset",
        },
        TimeRangeBoundary::Absolute(_) => match time_type {
            TimeType::DurationNs | TimeType::TimestampNs => "absolute time",
            TimeType::Sequence => "absolute frame",
        },
        TimeRangeBoundary::Infinite => {
            if low_bound {
                "beginning of timeline"
            } else {
                "end of timeline"
            }
        }
    }
}

#[expect(clippy::too_many_arguments)]
fn edit_boundary_ui(
    ui: &mut egui::Ui,
    boundary: &mut TimeRangeBoundary,
    time_type: TimeType,
    current_time: TimeInt,
    time_drag_value: &TimeDragValue,
    low_bound: bool,
    other_boundary_absolute: TimeInt,
    timestamp_format: TimestampFormat,
) {
    let (abs_time, rel_time) = match *boundary {
        TimeRangeBoundary::CursorRelative(time) => (time + current_time, time),
        TimeRangeBoundary::Absolute(time) => (time, time - current_time),
        TimeRangeBoundary::Infinite => (current_time, TimeInt(0)),
    };
    let abs_time = TimeRangeBoundary::Absolute(abs_time);
    let rel_time = TimeRangeBoundary::CursorRelative(rel_time);

    egui::ComboBox::from_id_salt(if low_bound {
        "time_history_low_bound"
    } else {
        "time_history_high_bound"
    })
    .selected_text(relative_time_range_boundary_label_text(
        *boundary, time_type, low_bound,
    ))
    .show_ui(ui, |ui| {
        ui.selectable_value(
            boundary,
            rel_time,
            relative_time_range_boundary_label_text(rel_time, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from a time point relative to the current time."
        } else {
            "Show data until a time point relative to the current time."
        });
        ui.selectable_value(
            boundary,
            abs_time,
            relative_time_range_boundary_label_text(abs_time, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from an absolute time point."
        } else {
            "Show data until an absolute time point."
        });
        ui.selectable_value(
            boundary,
            TimeRangeBoundary::Infinite,
            relative_time_range_boundary_label_text(
                TimeRangeBoundary::Infinite,
                time_type,
                low_bound,
            ),
        )
        .on_hover_text(if low_bound {
            "Show data from the beginning of the timeline"
        } else {
            "Show data until the end of the timeline"
        });
    });

    // Note: the time range adjustment below makes sure the two boundaries don't cross in time
    // (i.e. low > high). It does so by prioritizing the low boundary. Moving the low boundary
    // against the high boundary will displace the high boundary. On the other hand, the high
    // boundary cannot be moved against the low boundary. This asymmetry is intentional, and avoids
    // both boundaries fighting each other in some corner cases (when the user interacts with the
    // current time cursor)
    match boundary {
        TimeRangeBoundary::CursorRelative(value) => {
            // see note above
            let low_bound_override = if low_bound {
                Some(re_log_types::TimeInt::MIN)
            } else {
                Some((other_boundary_absolute - current_time).into())
            };

            let mut edit_value = (*value).into();
            time_drag_value
                .drag_value_ui(
                    ui,
                    time_type,
                    &mut edit_value,
                    false,
                    low_bound_override,
                    timestamp_format,
                )
                .on_hover_text(match time_type {
                    TimeType::DurationNs | TimeType::TimestampNs => {
                        "Time duration before/after the current time to use as a \
                         time range boundary"
                    }
                    TimeType::Sequence => {
                        "Number of frames before/after the current time to use a \
                         time range boundary"
                    }
                });

            *value = edit_value.into();
        }
        TimeRangeBoundary::Absolute(value) => {
            // see note above
            let low_bound_override = if low_bound {
                Some(re_log_types::TimeInt::MIN)
            } else {
                Some(other_boundary_absolute.into())
            };

            let mut edit_value = (*value).into();
            match time_type {
                TimeType::DurationNs | TimeType::TimestampNs => {
                    let (drag_resp, base_time_resp) = time_drag_value.temporal_drag_value_ui(
                        ui,
                        &mut edit_value,
                        true,
                        low_bound_override,
                        timestamp_format,
                    );

                    if let Some(base_time_resp) = base_time_resp {
                        base_time_resp.on_hover_text("Base time used to set time range boundaries");
                    }

                    drag_resp.on_hover_text("Absolute time to use as time range boundary");
                }
                TimeType::Sequence => {
                    time_drag_value
                        .sequence_drag_value_ui(ui, &mut edit_value, true, low_bound_override)
                        .on_hover_text("Absolute frame number to use as time range boundary");
                }
            }
            *value = edit_value.into();
        }
        TimeRangeBoundary::Infinite => {}
    }
}

/// Returns (label text, on hover text).
pub fn relative_time_range_label_text(
    current_time: TimeInt,
    time_type: TimeType,
    time_range: &TimeRange,
    timestamp_format: TimestampFormat,
) -> (String, Option<String>) {
    if time_range.start == TimeRangeBoundary::Infinite
        && time_range.end == TimeRangeBoundary::Infinite
    {
        ("Entire timeline".to_owned(), None)
    } else if time_range.start == TimeRangeBoundary::AT_CURSOR
        && time_range.end == TimeRangeBoundary::AT_CURSOR
    {
        let current_time = time_type.format(current_time, timestamp_format);
        (format!("At {current_time}"),

            Some("Does not perform a latest-at query, shows only data logged at exactly the current time cursor position.".to_owned()))
    } else {
        let absolute_range = AbsoluteTimeRange::from_relative_time_range(time_range, current_time);
        let from_formatted = time_type.format(absolute_range.min(), timestamp_format);
        let to_formatted = time_type.format(absolute_range.max(), timestamp_format);

        (
            format!("{from_formatted} to {to_formatted}"),
            Some("Showing data in this range (inclusive).".to_owned()),
        )
    }
}

impl RelativeTimeRange<'_> {
    pub fn ui(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let response_x = ui.list_item().interactive(false).show_hierarchical(
            ui,
            list_item::PropertyContent::new("start").value_fn(|ui, _| {
                edit_boundary_ui(
                    ui,
                    &mut self.value.start,
                    self.time_type,
                    self.current_time,
                    self.time_drag_value,
                    true,
                    self.resolved_range.max.into(),
                    self.timestamp_format,
                );
            }),
        );

        let response_y = ui.list_item().interactive(false).show_hierarchical(
            ui,
            list_item::PropertyContent::new("end").value_fn(|ui, _| {
                edit_boundary_ui(
                    ui,
                    &mut self.value.end,
                    self.time_type,
                    self.current_time,
                    self.time_drag_value,
                    false,
                    self.resolved_range.min.into(),
                    self.timestamp_format,
                );
            }),
        );

        let (text, on_hover) = relative_time_range_label_text(
            self.current_time,
            self.time_type,
            self.value,
            self.timestamp_format,
        );

        let mut response_z = ui
            .list_item()
            .interactive(false)
            .show_hierarchical(ui, LabelContent::new(text));

        if let Some(on_hover) = on_hover {
            response_z = response_z.on_hover_text(on_hover);
        }

        response_x | response_y | response_z
    }
}

#[cfg(test)]
mod tests {
    use std::ops::RangeInclusive;

    use egui_kittest::{Harness, SnapshotResults};
    use re_log_types::external::re_types_core::datatypes::{TimeInt, TimeRange, TimeRangeBoundary};
    use re_log_types::{TimeType, TimestampFormat};

    use super::RelativeTimeRange;
    use crate::{TimeDragValue, UiExt as _};

    struct SnapshotOptions {
        name: &'static str,
        current_time: i64,
        time_range: TimeRange,
    }

    fn run_snapshot(
        time_type: TimeType,
        timeline_range: RangeInclusive<i64>,
        timestamp_format: TimestampFormat,
        SnapshotOptions {
            name,
            current_time,
            mut time_range,
        }: SnapshotOptions,
        snapshot_results: &mut SnapshotResults,
    ) {
        let mut harness = Harness::builder().build_ui(|ui| {
            crate::apply_style_and_install_loaders(ui.ctx());

            let start = time_range.start.start_boundary_time(TimeInt(current_time));
            let end = time_range.end.end_boundary_time(TimeInt(current_time));

            ui.list_item_scope("test", |ui| {
                RelativeTimeRange {
                    time_drag_value: &TimeDragValue::from_time_range(timeline_range.clone()),
                    value: &mut time_range,
                    resolved_range: re_log_types::AbsoluteTimeRange {
                        min: start.into(),
                        max: end.into(),
                    },
                    time_type,
                    timestamp_format,
                    current_time: TimeInt(current_time),
                }
                .ui(ui);
            });
        });

        harness.fit_contents();
        harness.snapshot(format!("relative_time_range_{name}_{time_type}"));
        snapshot_results.extend_harness(&mut harness);
    }

    fn test_date_time(add_secs: i64) -> i64 {
        // 12345 days and 10 hours after unix epoch
        1_000_000_000 * (60 * 60 * (24 * 12345 + 10) + add_secs)
    }

    #[test]
    fn test_relative_time_range_ui() {
        let timestamp_format = TimestampFormat::utc().with_hide_today_date(true);
        let mut snapshot_results = SnapshotResults::new();
        for (time_type, time_range) in [
            (TimeType::Sequence, 0..=100),
            (
                TimeType::DurationNs,
                re_log_types::TimeInt::from_secs(0.0).as_i64()
                    ..=re_log_types::TimeInt::from_secs(60.0).as_i64(),
            ),
            (
                TimeType::TimestampNs,
                test_date_time(0)..=test_date_time(5000),
            ),
        ] {
            let start = *time_range.start();
            let end = *time_range.end();
            let middle = i64::midpoint(start, end);
            let after_start = i64::midpoint(start, middle);
            let before_end = i64::midpoint(middle, end);
            let sz = end - start;
            for o in [
                SnapshotOptions {
                    name: "everything",
                    current_time: start,
                    time_range: TimeRange::EVERYTHING,
                },
                SnapshotOptions {
                    name: "at_cursor",
                    current_time: middle,
                    time_range: TimeRange::AT_CURSOR,
                },
                SnapshotOptions {
                    name: "absolute",
                    current_time: start,
                    time_range: TimeRange {
                        start: TimeRangeBoundary::Absolute(TimeInt(after_start)),
                        end: TimeRangeBoundary::Absolute(TimeInt(before_end)),
                    },
                },
                SnapshotOptions {
                    name: "absolute_to_end",
                    current_time: start,
                    time_range: TimeRange {
                        start: TimeRangeBoundary::Absolute(TimeInt(after_start)),
                        end: TimeRangeBoundary::Infinite,
                    },
                },
                SnapshotOptions {
                    name: "start_to_absolute",
                    current_time: start,
                    time_range: TimeRange {
                        start: TimeRangeBoundary::Infinite,
                        end: TimeRangeBoundary::Absolute(TimeInt(before_end)),
                    },
                },
                SnapshotOptions {
                    name: "cursor_to_end",
                    current_time: before_end,
                    time_range: TimeRange {
                        start: TimeRangeBoundary::CursorRelative(TimeInt(-sz / 2)),
                        end: TimeRangeBoundary::Infinite,
                    },
                },
                SnapshotOptions {
                    name: "start_to_cursor",
                    current_time: after_start,
                    time_range: TimeRange {
                        start: TimeRangeBoundary::Infinite,
                        end: TimeRangeBoundary::CursorRelative(TimeInt(sz / 2)),
                    },
                },
                SnapshotOptions {
                    name: "around_cursor",
                    current_time: middle,
                    time_range: TimeRange {
                        start: TimeRangeBoundary::CursorRelative(TimeInt(-sz / 4)),
                        end: TimeRangeBoundary::CursorRelative(TimeInt(sz / 4)),
                    },
                },
            ] {
                run_snapshot(
                    time_type,
                    time_range.clone(),
                    timestamp_format,
                    o,
                    &mut snapshot_results,
                );
            }
        }
    }
}
