use egui::{CursorIcon, Id, NumExt as _, Rect};

use re_data_store::LogDb;
use re_log_types::{Duration, TimeInt, TimeRangeF, TimeReal, TimeType};
use re_viewer_context::{Looping, TimeControl};

use super::{is_time_safe_to_show, time_ranges_ui::TimeRangesUi};

pub fn loop_selection_ui(
    log_db: &LogDb,
    time_ctrl: &mut TimeControl,
    time_ranges_ui: &TimeRangesUi,
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    timeline_rect: &Rect,
) {
    let timeline = *time_ctrl.timeline();

    if time_ctrl.loop_selection().is_none() && time_ctrl.looping() == Looping::Selection {
        // Helpfully select a time slice
        if let Some(selection) = initial_time_selection(time_ranges_ui, time_ctrl.time_type()) {
            time_ctrl.set_loop_selection(selection);
        }
    }

    if time_ctrl.loop_selection().is_none() && time_ctrl.looping() == Looping::Selection {
        time_ctrl.set_looping(Looping::Off);
    }

    let is_active = time_ctrl.looping() == Looping::Selection;

    let selection_color = if is_active {
        re_ui::ReUi::loop_selection_color().gamma_multiply(0.7)
    } else {
        re_ui::ReUi::loop_selection_color().gamma_multiply(0.5)
    };

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let is_pointer_in_timeline =
        pointer_pos.map_or(false, |pointer_pos| timeline_rect.contains(pointer_pos));

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

            let full_y_range = rect.top()..=time_area_painter.clip_rect().bottom();

            if is_active {
                let full_rect = Rect::from_x_y_ranges(rect.x_range(), full_y_range);
                let rounding = re_ui::ReUi::normal_rounding();
                time_area_painter.rect_filled(full_rect, rounding, selection_color);
            } else {
                let rounding = re_ui::ReUi::normal_rounding();
                time_area_painter.rect_filled(rect, rounding, selection_color);
            }

            if is_active
                && !selected_range.is_empty()
                && is_time_safe_to_show(log_db, &timeline, selected_range.min)
                && is_time_safe_to_show(log_db, &timeline, selected_range.max)
            {
                paint_range_text(time_ctrl, selected_range, ui, time_area_painter, rect);
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

                // Check middle first, so that the edges "wins" (are on top)
                let middle_response = ui
                    .interact(rect, middle_id, egui::Sense::click_and_drag())
                    .on_hover_and_drag_cursor(CursorIcon::Move);

                let left_response = ui
                    .interact(left_edge_rect, left_edge_id, egui::Sense::drag())
                    .on_hover_and_drag_cursor(CursorIcon::ResizeWest);

                let right_response = ui
                    .interact(right_edge_rect, right_edge_id, egui::Sense::drag())
                    .on_hover_and_drag_cursor(CursorIcon::ResizeEast);

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

                if middle_response.dragged() {
                    on_drag_loop_selection(ui, time_ranges_ui, &mut selected_range);
                }
            } else {
                // inactive - show a tooltip at least:
                ui.interact(rect, middle_id, egui::Sense::hover())
                        .on_hover_text("Click the loop button to turn on the loop selection, or use shift-drag to select a new loop selection.");
            }
        }

        if selected_range.is_empty() && !ui.memory(|mem| mem.is_anything_being_dragged()) {
            // A zero-sized loop selection is confusing (and invisible), so remove it
            // (unless we are in the process of dragging right now):
            time_ctrl.remove_loop_selection();
        } else {
            // Update it in case it was modified:
            time_ctrl.set_loop_selection(selected_range);
        }
    }

    // Start new selection?
    if let Some(pointer_pos) = pointer_pos {
        let is_anything_being_dragged = ui.memory(|mem| mem.is_anything_being_dragged());
        if is_pointer_in_timeline
            && !is_anything_being_dragged
            && ui.input(|i| i.pointer.primary_down() && i.modifiers.shift_only())
        {
            if let Some(time) = time_ranges_ui.time_from_x_f32(pointer_pos.x) {
                time_ctrl.set_loop_selection(TimeRangeF::point(time));
                time_ctrl.set_looping(Looping::Selection);
                ui.memory_mut(|mem| mem.set_dragged_id(right_edge_id));
            }
        }
    }
}

fn initial_time_selection(
    time_ranges_ui: &TimeRangesUi,
    time_type: TimeType,
) -> Option<TimeRangeF> {
    let ranges = &time_ranges_ui.segments;

    // Try to find a long duration first, then fall back to shorter
    for min_duration in [2.0, 0.5, 0.0] {
        for segment in ranges {
            let range = &segment.tight_time;
            if range.min < range.max {
                match time_type {
                    TimeType::Time => {
                        let seconds = Duration::from(range.max - range.min).as_secs_f64();
                        if seconds > min_duration {
                            let one_sec = TimeInt::from(Duration::from_secs(1.0));
                            return Some(TimeRangeF::new(range.min, range.min + one_sec));
                        }
                    }
                    TimeType::Sequence => {
                        return Some(TimeRangeF::new(
                            range.min,
                            TimeReal::from(range.min)
                                + TimeReal::from((range.max - range.min).as_f64() / 2.0),
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
        Some(TimeRangeF::new(
            ranges[0].tight_time.min,
            ranges[end].tight_time.max,
        ))
    }
}

fn drag_right_loop_selection_edge(
    ui: &mut egui::Ui,
    time_ranges_ui: &TimeRangesUi,
    selected_range: &mut TimeRangeF,
    right_edge_id: Id,
) -> Option<()> {
    use egui::emath::smart_aim::best_in_range_f64;
    let pointer_pos = ui.input(|i| i.pointer.hover_pos())?;
    let aim_radius = ui.input(|i| i.aim_radius());

    let time_low = time_ranges_ui.time_from_x_f32(pointer_pos.x - aim_radius)?;
    let time_high = time_ranges_ui.time_from_x_f32(pointer_pos.x + aim_radius)?;

    // TODO(emilk): snap to absolute time too
    let low_length = selected_range.max - time_low;
    let high_length = selected_range.max - time_high;
    let best_length = TimeReal::from(best_in_range_f64(low_length.as_f64(), high_length.as_f64()));

    selected_range.min = selected_range.max - best_length;

    if selected_range.min > selected_range.max {
        std::mem::swap(&mut selected_range.min, &mut selected_range.max);
        ui.memory_mut(|mem| mem.set_dragged_id(right_edge_id));
    }

    Some(())
}

fn drag_left_loop_selection_edge(
    ui: &mut egui::Ui,
    time_ranges_ui: &TimeRangesUi,
    selected_range: &mut TimeRangeF,
    left_edge_id: Id,
) -> Option<()> {
    use egui::emath::smart_aim::best_in_range_f64;
    let pointer_pos = ui.input(|i| i.pointer.hover_pos())?;
    let aim_radius = ui.input(|i| i.aim_radius());

    let time_low = time_ranges_ui.time_from_x_f32(pointer_pos.x - aim_radius)?;
    let time_high = time_ranges_ui.time_from_x_f32(pointer_pos.x + aim_radius)?;

    // TODO(emilk): snap to absolute time too
    let low_length = time_low - selected_range.min;
    let high_length = time_high - selected_range.min;
    let best_length = TimeReal::from(best_in_range_f64(low_length.as_f64(), high_length.as_f64()));

    selected_range.max = selected_range.min + best_length;

    if selected_range.min > selected_range.max {
        std::mem::swap(&mut selected_range.min, &mut selected_range.max);
        ui.memory_mut(|mem| mem.set_dragged_id(left_edge_id));
    }

    Some(())
}

fn on_drag_loop_selection(
    ui: &mut egui::Ui,
    time_ranges_ui: &TimeRangesUi,
    selected_range: &mut TimeRangeF,
) -> Option<()> {
    let pointer_delta = ui.input(|i| i.pointer.delta());

    let min_x = time_ranges_ui.x_from_time_f32(selected_range.min)? + pointer_delta.x;
    let max_x = time_ranges_ui.x_from_time_f32(selected_range.max)? + pointer_delta.x;

    let min_time = time_ranges_ui.time_from_x_f32(min_x)?;
    let max_time = time_ranges_ui.time_from_x_f32(max_x)?;

    let mut new_range = TimeRangeF::new(min_time, max_time);

    if egui::emath::almost_equal(
        selected_range.length().as_f32(),
        new_range.length().as_f32(),
        1e-5,
    ) {
        // Avoid numerical inaccuracies: maintain length if very close
        new_range.max = new_range.min + selected_range.length();
    }

    *selected_range = new_range;

    Some(())
}

fn paint_range_text(
    time_ctrl: &mut TimeControl,
    selected_range: TimeRangeF,
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    selection_rect: Rect,
) {
    use egui::{Pos2, Stroke};

    if selected_range.min <= TimeInt::BEGINNING {
        return; // huge time selection, don't show a confusing times
    }

    let text_color = ui.visuals().strong_text_color();

    let arrow_color = text_color.gamma_multiply(0.75);
    let arrow_stroke = Stroke::new(1.0, arrow_color);

    fn paint_arrow_from_to(painter: &egui::Painter, origin: Pos2, to: Pos2, stroke: Stroke) {
        use egui::emath::Rot2;
        let vec = to - origin;
        let rot = Rot2::from_angle(std::f32::consts::TAU / 10.0);
        let tip_length = 6.0;
        let tip = origin + vec;
        let dir = vec.normalized();
        painter.line_segment([origin, tip], stroke);
        painter.line_segment([tip, tip - tip_length * (rot * dir)], stroke);
        painter.line_segment([tip, tip - tip_length * (rot.inverse() * dir)], stroke);
    }

    let range_text = format_duration(time_ctrl.time_type(), selected_range.length().abs());
    if range_text.is_empty() {
        return;
    }

    let font_id = egui::TextStyle::Small.resolve(ui.style());
    let text_rect = painter.text(
        selection_rect.center(),
        egui::Align2::CENTER_CENTER,
        range_text,
        font_id,
        text_color,
    );

    // Draw arrows on either side, if we have the space for it:
    let text_rect = text_rect.expand(2.0); // Add some margin around text
    let selection_rect = selection_rect.shrink(1.0); // Add some margin inside of the selection rect
    let min_arrow_length = 12.0;
    if selection_rect.left() + min_arrow_length <= text_rect.left() {
        paint_arrow_from_to(
            painter,
            text_rect.left_center(),
            selection_rect.left_center(),
            arrow_stroke,
        );
    }
    if text_rect.right() + min_arrow_length <= selection_rect.right() {
        paint_arrow_from_to(
            painter,
            text_rect.right_center(),
            selection_rect.right_center(),
            arrow_stroke,
        );
    }
}

/// Human-readable description of a duration
fn format_duration(time_typ: TimeType, duration: TimeReal) -> String {
    match time_typ {
        TimeType::Time => Duration::from(duration).to_string(),
        TimeType::Sequence => duration.round().as_i64().to_string(), // TODO(emilk): show real part?
    }
}
