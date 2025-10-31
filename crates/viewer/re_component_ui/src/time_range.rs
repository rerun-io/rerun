use egui::NumExt as _;
use re_log_types::{AbsoluteTimeRange, TimeType};
use re_types::{
    blueprint::components::TimeRange,
    datatypes::{TimeInt, TimeRangeBoundary},
};
use re_ui::{TimeDragValue, UiExt as _, list_item::LabelContent};
use re_viewer_context::{MaybeMutRef, TimeControlCommand};

pub fn time_range_multiline_edit_or_view_ui(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, TimeRange>,
) -> egui::Response {
    let time_drag_value = if let Some(times) = ctx
        .recording()
        .time_histogram(ctx.time_ctrl.timeline().name())
    {
        TimeDragValue::from_time_histogram(times)
    } else {
        TimeDragValue::from_time_range(0..=0)
    };

    let current_time = TimeInt(
        ctx.time_ctrl
            .time_i64()
            .unwrap_or_default()
            .at_least(*time_drag_value.range.start()),
    ); // accounts for static time (TimeInt::MIN)

    let current_start = value.start.start_boundary_time(current_time);
    let current_end = value.end.end_boundary_time(current_time);

    let time_type = ctx.time_ctrl.time_type();

    let mut any_edit = false;

    let response_x = ui.list_item().interactive(false).show_hierarchical(
        ui,
        re_ui::list_item::PropertyContent::new("start").value_fn(|ui, _| {
            if let Some(value) = value.as_mut() {
                let old_start = value.start;

                edit_visible_history_boundary_ui(
                    ctx,
                    ui,
                    &mut value.start,
                    time_type,
                    current_time,
                    &time_drag_value,
                    true,
                    current_end,
                );

                any_edit |= old_start != value.start;
            } else {
                view_visible_history_boundary_ui(ctx, ui, &value.start, time_type, true);
            }
        }),
    );

    let response_y = ui.list_item().interactive(false).show_hierarchical(
        ui,
        re_ui::list_item::PropertyContent::new("end").value_fn(|ui, _| {
            if let Some(value) = value.as_mut() {
                let old_end = value.end;

                edit_visible_history_boundary_ui(
                    ctx,
                    ui,
                    &mut value.end,
                    time_type,
                    current_time,
                    &time_drag_value,
                    false,
                    current_start,
                );

                any_edit |= old_end != value.end;
            } else {
                view_visible_history_boundary_ui(ctx, ui, &value.start, time_type, false);
            }
        }),
    );

    let (text, on_hover) = current_range_label(ctx, current_time, time_type, value, false);

    let mut response_z = ui
        .list_item()
        .interactive(false)
        .show_hierarchical(ui, LabelContent::new(text));

    if let Some(on_hover) = on_hover {
        response_z = response_z.on_hover_text(on_hover);
    }

    let mut response = response_x | response_y | response_z;
    if any_edit {
        response.mark_changed();
    }

    if ui.rect_contains_pointer(response.rect) {
        let absolute_range = AbsoluteTimeRange::from_relative_time_range(value, current_time);
        ctx.send_time_commands([TimeControlCommand::HighlightRange(absolute_range)]);
    }

    response
}

pub fn time_range_singleline_view_ui(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, TimeRange>,
) -> egui::Response {
    let time_drag_value = if let Some(times) = ctx
        .recording()
        .time_histogram(ctx.time_ctrl.timeline().name())
    {
        TimeDragValue::from_time_histogram(times)
    } else {
        TimeDragValue::from_time_range(0..=0)
    };

    let current_time = TimeInt(
        ctx.time_ctrl
            .time_i64()
            .unwrap_or_default()
            .at_least(*time_drag_value.range.start()),
    ); // accounts for static time (TimeInt::MIN)

    let time_type = ctx.time_ctrl.time_type();

    let (text, on_hover) = current_range_label(ctx, current_time, time_type, value, true);

    let mut res = ui.label(text);

    if let Some(on_hover) = on_hover {
        res = res.on_hover_text(on_hover);
    }

    if res.hovered() {
        let absolute_range = AbsoluteTimeRange::from_relative_time_range(value, current_time);
        ctx.send_time_commands([TimeControlCommand::HighlightRange(absolute_range)]);
    }

    res
}

/// Returns (label text, on hover text).
fn current_range_label(
    ctx: &re_viewer_context::ViewerContext<'_>,
    current_time: TimeInt,
    time_type: TimeType,
    time_range: &TimeRange,
    short_date: bool,
) -> (String, Option<String>) {
    if time_range.start == TimeRangeBoundary::Infinite
        && time_range.end == TimeRangeBoundary::Infinite
    {
        ("Entire timeline".to_owned(), None)
    } else if time_range.start == TimeRangeBoundary::AT_CURSOR
        && time_range.end == TimeRangeBoundary::AT_CURSOR
    {
        let current_time = time_type.format(
            current_time,
            ctx.app_options().timestamp_format.with_short(short_date),
        );
        (format!("At {} = {current_time}", ctx.time_ctrl.timeline().name()),

            Some("Does not perform a latest-at query, shows only data logged at exactly the current time cursor position.".to_owned()))
    } else {
        let absolute_range = AbsoluteTimeRange::from_relative_time_range(time_range, current_time);
        let from_formatted = time_type.format(
            absolute_range.min(),
            ctx.app_options().timestamp_format.with_short(short_date),
        );
        let to_formatted = time_type.format(
            absolute_range.max(),
            ctx.app_options().timestamp_format.with_short(short_date),
        );

        (
            format!("{from_formatted} to {to_formatted}"),
            Some("Showing data in this range (inclusive).".to_owned()),
        )
    }
}

fn visible_history_boundary_combo_label(
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

fn view_visible_history_boundary_ui(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    visible_history_boundary: &TimeRangeBoundary,
    time_type: TimeType,
    low_bound: bool,
) {
    ui.label(visible_history_boundary_combo_label(
        *visible_history_boundary,
        time_type,
        low_bound,
    ));

    match visible_history_boundary {
        TimeRangeBoundary::CursorRelative(time_int) => {
            ui.label(
                match time_type {
                    TimeType::Sequence => TimeType::Sequence,
                    TimeType::DurationNs | TimeType::TimestampNs => TimeType::DurationNs,
                }
                .format(*time_int, ctx.app_options().timestamp_format),
            );
        }
        TimeRangeBoundary::Absolute(time_int) => {
            ui.label(time_type.format(*time_int, ctx.app_options().timestamp_format));
        }
        TimeRangeBoundary::Infinite => {}
    }
}

#[expect(clippy::too_many_arguments)]
fn edit_visible_history_boundary_ui(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    visible_history_boundary: &mut TimeRangeBoundary,
    time_type: TimeType,
    current_time: TimeInt,
    time_drag_value: &TimeDragValue,
    low_bound: bool,
    other_boundary_absolute: TimeInt,
) {
    let (abs_time, rel_time) = match *visible_history_boundary {
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
    .selected_text(visible_history_boundary_combo_label(
        *visible_history_boundary,
        time_type,
        low_bound,
    ))
    .show_ui(ui, |ui| {
        ui.selectable_value(
            visible_history_boundary,
            rel_time,
            visible_history_boundary_combo_label(rel_time, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from a time point relative to the current time."
        } else {
            "Show data until a time point relative to the current time."
        });
        ui.selectable_value(
            visible_history_boundary,
            abs_time,
            visible_history_boundary_combo_label(abs_time, time_type, low_bound),
        )
        .on_hover_text(if low_bound {
            "Show data from an absolute time point."
        } else {
            "Show data until an absolute time point."
        });
        ui.selectable_value(
            visible_history_boundary,
            TimeRangeBoundary::Infinite,
            visible_history_boundary_combo_label(TimeRangeBoundary::Infinite, time_type, low_bound),
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
    match visible_history_boundary {
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
                    ctx.app_options().timestamp_format,
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
                        ctx.app_options().timestamp_format,
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
