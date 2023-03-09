//! Show the data density over time for a data stream.

use std::collections::BTreeMap;

use egui::{NumExt as _, Rect, Shape};
use itertools::Itertools as _;

use re_log_types::{TimeInt, TimeRange, TimeReal};

use crate::{
    misc::{Item, ViewerContext},
    ui::{time_panel::ball_scatterer::BallScatterer, Blueprint},
};

use super::time_ranges_ui::TimeRangesUi;

struct Stretch {
    start_x: f32,
    time_range: TimeRange,
    selected: bool,
    /// Times x count at the given time
    time_points: Vec<(TimeInt, usize)>,
}

#[allow(clippy::too_many_arguments)]
pub fn show_data_over_time(
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    time_area_response: &egui::Response,
    time_area_painter: &egui::Painter,
    ui: &mut egui::Ui,
    num_timeless_messages: usize,
    num_messages_at_time: &BTreeMap<TimeInt, usize>,
    full_width_rect: Rect,
    time_ranges_ui: &TimeRangesUi,
    select_on_click: Item,
) {
    crate::profile_function!();

    // TODO(andreas): Should pass through underlying instance id and be clever about selection vs hover state.
    let is_selected = ctx.selection().iter().contains(&select_on_click);

    // painting each data point as a separate circle is slow (too many circles!)
    // so we join time points that are close together.
    let points_per_time = time_ranges_ui.points_per_time().unwrap_or(f64::INFINITY);
    let max_stretch_length_in_time = 1.0 / points_per_time; // TODO(emilk)

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    let hovered_color = ui.visuals().widgets.hovered.text_color();
    let inactive_color = if is_selected {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals()
            .widgets
            .inactive
            .text_color()
            .linear_multiply(0.75)
    };

    let mut shapes = vec![];
    let mut scatter = BallScatterer::default();
    // Time x number of messages at that time point
    let mut hovered_messages: Vec<(TimeInt, usize)> = vec![];
    let mut hovered_time = None;

    let mut paint_stretch = |stretch: &Stretch| {
        let stop_x = time_ranges_ui
            .x_from_time_f32(stretch.time_range.max.into())
            .unwrap_or(stretch.start_x);

        let num_messages: usize = stretch.time_points.iter().map(|(_time, count)| count).sum();
        let radius = 2.5 * (1.0 + 0.5 * (num_messages as f32).log10());
        let radius = radius.at_most(full_width_rect.height() / 3.0);
        debug_assert!(radius.is_finite());

        let x = (stretch.start_x + stop_x) * 0.5;
        let pos = scatter.add(x, radius, (full_width_rect.top(), full_width_rect.bottom()));

        let is_hovered = pointer_pos.map_or(false, |pointer_pos| {
            pos.distance(pointer_pos) < radius + 1.0
        });

        let mut color = if is_hovered {
            hovered_color
        } else {
            inactive_color
        };
        if ui.visuals().dark_mode {
            color = color.additive();
        }

        let radius = if is_hovered {
            1.75 * radius
        } else if stretch.selected {
            1.25 * radius
        } else {
            radius
        };

        shapes.push(Shape::circle_filled(pos, radius, color));

        if is_hovered {
            hovered_messages.extend(stretch.time_points.iter().copied());
            hovered_time.get_or_insert(stretch.time_range.min);
        }
    };

    let selected_time_range = ctx.rec_cfg.time_ctrl.active_loop_selection();

    if num_timeless_messages > 0 {
        let time_int = TimeInt::BEGINNING;
        let time_real = TimeReal::from(time_int);
        if let Some(x) = time_ranges_ui.x_from_time_f32(time_real) {
            let selected = selected_time_range.map_or(true, |range| range.contains(time_real));
            paint_stretch(&Stretch {
                start_x: x,
                time_range: TimeRange::point(time_int),
                selected,
                time_points: vec![(time_int, num_timeless_messages)],
            });
        }
    }

    let mut stretch: Option<Stretch> = None;

    let margin = 5.0;
    let visible_time_range = TimeRange {
        min: time_ranges_ui
            .time_from_x_f32(time_area_painter.clip_rect().left() - margin)
            .map_or(TimeInt::MIN, |tf| tf.floor()),

        max: time_ranges_ui
            .time_from_x_f32(time_area_painter.clip_rect().right() + margin)
            .map_or(TimeInt::MAX, |tf| tf.ceil()),
    };

    for (&time, &num_messages_at_time) in
        num_messages_at_time.range(visible_time_range.min..=visible_time_range.max)
    {
        if num_messages_at_time == 0 {
            continue;
        }
        let time_real = TimeReal::from(time);

        let selected = selected_time_range.map_or(true, |range| range.contains(time_real));

        if let Some(current_stretch) = &mut stretch {
            if current_stretch.selected == selected
                && (time - current_stretch.time_range.min).as_f64() < max_stretch_length_in_time
            {
                // extend:
                current_stretch.time_range.max = time;
                current_stretch
                    .time_points
                    .push((time, num_messages_at_time));
            } else {
                // stop the previous…
                paint_stretch(current_stretch);

                stretch = None;
            }
        }

        if stretch.is_none() {
            if let Some(x) = time_ranges_ui.x_from_time_f32(time_real) {
                stretch = Some(Stretch {
                    start_x: x,
                    time_range: TimeRange::point(time),
                    selected,
                    time_points: vec![(time, num_messages_at_time)],
                });
            }
        }
    }

    if let Some(stretch) = stretch {
        paint_stretch(&stretch);
    }

    time_area_painter.extend(shapes);

    if !hovered_messages.is_empty() {
        if time_area_response.clicked_by(egui::PointerButton::Primary) {
            ctx.set_single_selection(select_on_click);

            if let Some(hovered_time) = hovered_time {
                ctx.rec_cfg.time_ctrl.set_time(hovered_time);
                ctx.rec_cfg.time_ctrl.pause();
            }
        } else if !ui.ctx().memory(|mem| mem.is_anything_being_dragged()) {
            show_msg_ids_tooltip(
                ctx,
                blueprint,
                ui.ctx(),
                &select_on_click,
                &hovered_messages,
            );
        }
    }
}

fn show_msg_ids_tooltip(
    ctx: &mut ViewerContext<'_>,
    blueprint: &mut Blueprint,
    egui_ctx: &egui::Context,
    item: &Item,
    time_points: &[(TimeInt, usize)],
) {
    use crate::ui::data_ui::DataUi as _;

    egui::show_tooltip_at_pointer(egui_ctx, egui::Id::new("data_tooltip"), |ui| {
        let num_times = time_points.len();
        let num_messages: usize = time_points.iter().map(|(_time, count)| *count).sum();

        if num_times == 1 {
            if num_messages > 1 {
                ui.label(format!("{num_messages} messages"));
                ui.add_space(8.0);
                // Could be an entity made up of many components logged at the same time.
                // Still show a preview!
            }
            crate::ui::selection_panel::what_is_selected_ui(ui, ctx, blueprint, item);
            ui.add_space(8.0);

            let timeline = *ctx.rec_cfg.time_ctrl.timeline();
            let time_int = time_points[0].0; // We want to show the item at the time of whatever point we are hovering
            let query = re_arrow_store::LatestAtQuery::new(timeline, time_int);
            item.data_ui(ctx, ui, crate::ui::UiVerbosity::Reduced, &query);
        } else {
            ui.label(format!(
                "{num_messages} messages at {num_times} points in time"
            ));
        }
    });
}
