use egui::NumExt as _;
use re_log_types::{AbsoluteTimeRange, TimeType};
use re_sdk_types::blueprint::components::TimeRange;
use re_sdk_types::datatypes::{TimeInt, TimeRangeBoundary};
use re_ui::list_item::LabelContent;
use re_ui::{
    RelativeTimeRange, TimeDragValue, UiExt as _, relative_time_range_boundary_label_text,
    relative_time_range_label_text,
};
use re_viewer_context::{MaybeMutRef, TimeControlCommand};

pub fn time_range_multiline_edit_or_view_ui(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, TimeRange>,
) -> egui::Response {
    let Some(time_type) = ctx.time_ctrl.time_type() else {
        return ui.weak("No active timeline");
    };

    let time_drag_value = if let Some(range) = ctx
        .recording()
        .time_range_for(ctx.time_ctrl.timeline_name())
    {
        TimeDragValue::from_abs_time_range(range)
    } else {
        TimeDragValue::from_time_range(0..=0)
    };

    let current_time = TimeInt(
        ctx.time_ctrl
            .time_i64()
            .unwrap_or_default()
            .at_least(*time_drag_value.range.start()),
    ); // accounts for static time (TimeInt::MIN)

    let response = match value {
        MaybeMutRef::Ref(value) => {
            let response_x = ui.list_item().interactive(false).show_hierarchical(
                ui,
                re_ui::list_item::PropertyContent::new("start").value_fn(|ui, _| {
                    view_visible_history_boundary_ui(ctx, ui, &value.start, time_type, true);
                }),
            );

            let response_y = ui.list_item().interactive(false).show_hierarchical(
                ui,
                re_ui::list_item::PropertyContent::new("end").value_fn(|ui, _| {
                    view_visible_history_boundary_ui(ctx, ui, &value.start, time_type, false);
                }),
            );

            let (text, on_hover) = relative_time_range_label_text(
                current_time,
                time_type,
                value,
                ctx.app_options().timestamp_format,
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
        MaybeMutRef::MutRef(value) => {
            let current_start = value.start.start_boundary_time(current_time);
            let current_end = value.end.end_boundary_time(current_time);

            let old_value = value.clone();

            let mut response = RelativeTimeRange {
                time_drag_value: &time_drag_value,
                value,
                resolved_range: AbsoluteTimeRange::new(current_start, current_end),
                time_type,
                timestamp_format: ctx.app_options().timestamp_format,
                current_time,
            }
            .ui(ui);

            if **value != old_value {
                response.mark_changed();
            }

            response
        }
    };

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
    let Some(time_type) = ctx.time_ctrl.time_type() else {
        return ui.weak("No active timeline");
    };

    let time_drag_value = if let Some(range) = ctx
        .recording()
        .time_range_for(ctx.time_ctrl.timeline_name())
    {
        TimeDragValue::from_abs_time_range(range)
    } else {
        TimeDragValue::from_time_range(0..=0)
    };

    let current_time = TimeInt(
        ctx.time_ctrl
            .time_i64()
            .unwrap_or_default()
            .at_least(*time_drag_value.range.start()),
    ); // accounts for static time (TimeInt::MIN)

    let (text, on_hover) = relative_time_range_label_text(
        current_time,
        time_type,
        value,
        ctx.app_options().timestamp_format.with_short(true),
    );

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

fn view_visible_history_boundary_ui(
    ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    visible_history_boundary: &TimeRangeBoundary,
    time_type: TimeType,
    low_bound: bool,
) {
    ui.label(relative_time_range_boundary_label_text(
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
