use egui::{Color32, CursorIcon, Id, NumExt as _, Rect};

use re_log_types::{
    AbsoluteTimeRange, AbsoluteTimeRangeF, Duration, TimeInt, TimeReal, TimeType, TimestampFormat,
};
use re_types::blueprint::components::LoopMode;
use re_ui::{UiExt as _, list_item};
use re_viewer_context::{TimeControl, TimeControlCommand, ViewerContext};

use super::time_ranges_ui::TimeRangesUi;

/// Paints a rect on the timeline given a time range.
pub fn paint_timeline_range(
    highlighted_range: AbsoluteTimeRange,
    time_ranges_ui: &TimeRangesUi,
    painter: &egui::Painter,
    rect: Rect,
    color: Color32,
) {
    let x_from = time_ranges_ui.x_from_time_f32(highlighted_range.min().into());
    let x_to = time_ranges_ui.x_from_time_f32(highlighted_range.max().into());

    if let (Some(x_from), Some(x_to)) = (x_from, x_to) {
        let visible_history_area_rect =
            Rect::from_x_y_ranges(x_from..=x_to, rect.y_range()).intersect(rect);

        painter.rect_filled(visible_history_area_rect, 0.0, color);
    }
}

pub fn collapsed_loop_selection_ui(
    time_ctrl: &TimeControl,
    painter: &egui::Painter,
    time_ranges_ui: &TimeRangesUi,
    ui: &egui::Ui,
    time_range_rect: Rect,
) {
    if let Some(loop_range) = time_ctrl.loop_selection() {
        let color = if time_ctrl.loop_mode() == LoopMode::Selection {
            ui.tokens().loop_selection_color
        } else {
            ui.tokens().loop_selection_color_inactive
        };
        paint_timeline_range(
            loop_range.to_int(),
            time_ranges_ui,
            painter,
            time_range_rect,
            color,
        );
    }
}

pub fn loop_selection_ui(
    ctx: &ViewerContext<'_>,
    time_ctrl: &TimeControl,
    time_ranges_ui: &TimeRangesUi,
    ui: &egui::Ui,
    time_area_painter: &egui::Painter,
    timeline_rect: &Rect,
    time_commands: &mut Vec<TimeControlCommand>,
) {
    let tokens = ui.tokens();

    if time_ctrl.loop_selection().is_none() && time_ctrl.loop_mode() == LoopMode::Selection {
        // Helpfully select a time slice
        if let Some(selection) = initial_time_selection(time_ranges_ui, time_ctrl.time_type()) {
            time_commands.push(TimeControlCommand::SetLoopSelection(selection.to_int()));
        }
    }

    if time_ctrl.loop_selection().is_none() && time_ctrl.loop_mode() == LoopMode::Selection {
        time_commands.push(TimeControlCommand::SetLoopMode(LoopMode::Off));
    }

    let is_active = time_ctrl.loop_mode() == LoopMode::Selection;

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    let timeline_response = ui
        .interact(
            *timeline_rect,
            ui.id().with("timeline"),
            egui::Sense::drag(),
        )
        .on_hover_cursor(crate::CREATE_TIME_LOOP_CURSOR_ICON);

    let left_edge_id = ui.id().with("selection_left_edge");
    let right_edge_id = ui.id().with("selection_right_edge");
    let middle_id = ui.id().with("selection_move");

    let interact_radius = ui.style().interaction.resize_grab_radius_side;

    // Paint existing selection and detect drag starting and hovering:
    if let Some(mut selected_range) = time_ctrl.loop_selection() {
        let min_x = time_ranges_ui.x_from_time(selected_range.min);
        let max_x = time_ranges_ui.x_from_time(selected_range.max);

        if let (Some(min_x), Some(max_x)) = (min_x, max_x) {
            // The top part only
            let mut rect =
                Rect::from_x_y_ranges((min_x as f32)..=(max_x as f32), timeline_rect.y_range());

            // Make sure it is visible:
            if rect.width() < 2.0 {
                rect = Rect::from_x_y_ranges(
                    (rect.center().x - 1.0)..=(rect.center().x - 1.0),
                    rect.y_range(),
                );
            }

            let full_y_range =
                egui::Rangef::new(rect.top(), time_area_painter.clip_rect().bottom());

            {
                let corner_radius = tokens.normal_corner_radius();
                let corner_radius = egui::CornerRadius {
                    nw: corner_radius,
                    ne: corner_radius,
                    sw: 0,
                    se: 0,
                };

                let full_color = tokens.loop_selection_color;
                let inactive_color = tokens.loop_selection_color_inactive;

                if is_active {
                    let full_rect = Rect::from_x_y_ranges(rect.x_range(), full_y_range);
                    time_area_painter.rect_filled(full_rect, corner_radius, full_color);
                } else {
                    time_area_painter.rect_filled(rect, corner_radius, full_color);

                    let bottom_rect =
                        Rect::from_x_y_ranges(rect.x_range(), rect.bottom()..=full_y_range.max);
                    time_area_painter.rect_filled(bottom_rect, 0.0, inactive_color);
                }
            }

            // Check for interaction:
            // To not annoy the user, we only allow interaction when it is active.
            if is_active {
                let left_edge_rect =
                    Rect::from_x_y_ranges(rect.left()..=rect.left(), rect.y_range())
                        .expand(interact_radius);

                let right_edge_rect =
                    Rect::from_x_y_ranges(rect.right()..=rect.right(), rect.y_range())
                        .expand(interact_radius);

                let time_type = time_ctrl.time_type();

                // Check middle first, so that the edges "wins" (are on top)
                let middle_response = ui
                    .interact(rect, middle_id, egui::Sense::click_and_drag())
                    .on_hover_and_drag_cursor(CursorIcon::Move)
                    .on_hover_ui_at_pointer(|ui| {
                        TimeLoopPart::Middle.tooltip_ui(
                            ui,
                            time_type,
                            selected_range,
                            ctx.app_options().timestamp_format,
                        );
                    });

                let left_response = ui
                    .interact(left_edge_rect, left_edge_id, egui::Sense::drag())
                    .on_hover_and_drag_cursor(CursorIcon::ResizeWest)
                    .on_hover_ui_at_pointer(|ui| {
                        TimeLoopPart::Beginning.tooltip_ui(
                            ui,
                            time_type,
                            selected_range,
                            ctx.app_options().timestamp_format,
                        );
                    });

                let right_response = ui
                    .interact(right_edge_rect, right_edge_id, egui::Sense::drag())
                    .on_hover_and_drag_cursor(CursorIcon::ResizeEast)
                    .on_hover_ui_at_pointer(|ui| {
                        TimeLoopPart::End.tooltip_ui(
                            ui,
                            time_type,
                            selected_range,
                            ctx.app_options().timestamp_format,
                        );
                    });

                if left_response.dragged() {
                    drag_right_loop_selection_edge(
                        ui,
                        time_ranges_ui,
                        &mut selected_range,
                        right_edge_id,
                    );
                }

                if right_response.dragged() {
                    drag_left_loop_selection_edge(
                        ui,
                        time_ranges_ui,
                        &mut selected_range,
                        left_edge_id,
                    );
                }

                on_drag_loop_selection(ui, &middle_response, time_ranges_ui, &mut selected_range);
            } else {
                // inactive - show a tooltip at least:
                ui.interact(rect, middle_id, egui::Sense::hover())
                    .on_hover_text("Click the loop button to turn on the loop selection, or drag to select a new loop selection");
            }
        }

        if selected_range.is_empty() && ui.ctx().dragged_id().is_none() {
            // A zero-sized loop selection is confusing (and invisible), so remove it
            // (unless we are in the process of dragging right now):
            time_commands.push(TimeControlCommand::RemoveLoopSelection);
        } else if Some(selected_range.to_int()) != time_ctrl.loop_selection().map(|s| s.to_int()) {
            // Update it if it was modified:
            time_commands.push(TimeControlCommand::SetLoopSelection(
                selected_range.to_int(),
            ));
        }
    }

    // Start new selection?
    if let Some(pointer_pos) = pointer_pos
        && timeline_response.hovered()
        && let Some(time) = time_ranges_ui.snapped_time_from_x(ui, pointer_pos.x)
    {
        // Show preview:
        let x = time_ranges_ui
            .x_from_time_f32(time)
            .unwrap_or(pointer_pos.x);
        ui.painter().vline(
            x,
            timeline_rect.y_range(),
            ui.visuals().widgets.noninteractive.fg_stroke,
        );

        if timeline_response.dragged() && ui.input(|i| i.pointer.is_decidedly_dragging()) {
            // Start new selection
            time_commands.push(TimeControlCommand::SetLoopSelection(
                AbsoluteTimeRangeF::point(time).to_int(),
            ));
            time_commands.push(TimeControlCommand::SetLoopMode(LoopMode::Selection));
            ui.ctx().set_dragged_id(right_edge_id);
        }
    }
}

/// What part of the time loop selection is the user hovering?
#[derive(Clone, Copy, Debug, Hash)]
enum TimeLoopPart {
    Beginning,
    Middle,
    End,
}

impl TimeLoopPart {
    pub fn tooltip_ui(
        &self,
        ui: &mut egui::Ui,
        time_type: TimeType,
        range: AbsoluteTimeRangeF,
        timestamp_format: TimestampFormat,
    ) {
        let range = range.to_int();
        list_item::list_item_scope(ui, self, |ui| {
            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Start")
                    .value_text(time_type.format(range.min, timestamp_format)),
            );
            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Stop")
                    .value_text(time_type.format(range.max, timestamp_format)),
            );

            let length = i64::try_from(range.abs_length()).unwrap_or(i64::MAX);
            ui.list_item_flat_noninteractive(
                list_item::PropertyContent::new("Length")
                    .value_text(format_duration(time_type, length.into())),
            );
        });
    }
}

fn initial_time_selection(
    time_ranges_ui: &TimeRangesUi,
    time_type: TimeType,
) -> Option<AbsoluteTimeRangeF> {
    let ranges = &time_ranges_ui.segments;

    // Try to find a long duration first, then fall back to shorter
    for min_duration in [2.0, 0.5, 0.0] {
        for segment in ranges {
            let range = &segment.tight_time;
            if range.min() < range.max() {
                match time_type {
                    TimeType::DurationNs | TimeType::TimestampNs => {
                        let seconds = Duration::from(range.max() - range.min()).as_secs_f64();
                        if seconds > min_duration {
                            let one_sec =
                                TimeInt::new_temporal(Duration::from_secs(1.0).as_nanos());
                            return Some(AbsoluteTimeRangeF::new(
                                range.min(),
                                range.min() + one_sec,
                            ));
                        }
                    }
                    TimeType::Sequence => {
                        return Some(AbsoluteTimeRangeF::new(
                            range.min(),
                            TimeReal::from(range.min())
                                + TimeReal::from((range.max() - range.min()).as_f64() / 2.0),
                        ));
                    }
                }
            }
        }
    }

    // all ranges have just a single data point in it. sight

    if ranges.len() < 2 {
        None // not enough to show anything meaningful
    } else {
        let end = (ranges.len() / 2).at_least(1);
        Some(AbsoluteTimeRangeF::new(
            ranges[0].tight_time.min(),
            ranges[end].tight_time.max(),
        ))
    }
}

fn drag_right_loop_selection_edge(
    ui: &egui::Ui,
    time_ranges_ui: &TimeRangesUi,
    selected_range: &mut AbsoluteTimeRangeF,
    right_edge_id: Id,
) -> Option<()> {
    let pointer_pos = ui.input(|i| i.pointer.hover_pos())?;
    let time = time_ranges_ui.snapped_time_from_x(ui, pointer_pos.x)?;
    selected_range.min = time;

    if selected_range.min > selected_range.max {
        std::mem::swap(&mut selected_range.min, &mut selected_range.max);
        ui.ctx().set_dragged_id(right_edge_id);
    }

    Some(())
}

fn drag_left_loop_selection_edge(
    ui: &egui::Ui,
    time_ranges_ui: &TimeRangesUi,
    selected_range: &mut AbsoluteTimeRangeF,
    left_edge_id: Id,
) -> Option<()> {
    let pointer_pos = ui.input(|i| i.pointer.hover_pos())?;
    let time = time_ranges_ui.snapped_time_from_x(ui, pointer_pos.x)?;
    selected_range.max = time;

    if selected_range.min > selected_range.max {
        std::mem::swap(&mut selected_range.min, &mut selected_range.max);
        ui.ctx().set_dragged_id(left_edge_id);
    }

    Some(())
}

fn on_drag_loop_selection(
    ui: &egui::Ui,
    drag_response: &egui::Response,
    time_ranges_ui: &TimeRangesUi,
    selected_range: &mut AbsoluteTimeRangeF,
) -> Option<()> {
    // Since we may snap time values, we need to store full-precision "unsnapped" value
    // somewhere, or we will accumulate rounding errors.
    let precise_min_id = ui.id().with("__time_loop_drag");

    if ui.input(|i| i.pointer.any_pressed() || i.pointer.any_released()) {
        ui.data_mut(|data| data.remove::<TimeReal>(precise_min_id));
    }

    if drag_response.dragged() {
        let old_precise_min = ui
            .data_mut(|data| data.get_temp::<TimeReal>(precise_min_id))
            .unwrap_or(selected_range.min);

        *selected_range = selected_range.to_int().into();

        let pointer_delta = ui.input(|i| i.pointer.delta());

        // We move the time selection in a way to preserve the length of it (in time units).
        // If there are gaps in the timeline, this can cause the _visual_ length of the
        // time selection to change. But that is the least worst option.

        let new_precise_min_x =
            time_ranges_ui.x_from_time(old_precise_min)? + pointer_delta.x as f64;
        let new_precise_min = time_ranges_ui.time_from_x_f64(new_precise_min_x)?;

        ui.data_mut(|data| data.insert_temp::<TimeReal>(precise_min_id, new_precise_min));

        let snapped_min = time_ranges_ui
            .snapped_time_from_x(ui, new_precise_min_x as f32)?
            .round();

        *selected_range =
            AbsoluteTimeRange::new(snapped_min, snapped_min + selected_range.length().round())
                .into();
    }

    Some(())
}

/// Human-readable description of a duration
fn format_duration(time_typ: TimeType, duration: TimeReal) -> String {
    match time_typ {
        TimeType::DurationNs | TimeType::TimestampNs => Duration::from(duration).to_string(),
        TimeType::Sequence => re_format::format_int(duration.round().as_i64()),
    }
}
