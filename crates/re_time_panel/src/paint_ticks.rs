use std::ops::RangeInclusive;

use egui::{lerp, pos2, remap_clamp, Align2, Color32, Rect, Rgba, Shape, Stroke};

use re_log_types::{Time, TimeRangeF, TimeReal, TimeType};

use crate::format_time::{format_time_compact, next_grid_tick_magnitude_ns};

use super::time_ranges_ui::TimeRangesUi;

pub fn paint_time_ranges_and_ticks(
    time_ranges_ui: &TimeRangesUi,
    ui: &mut egui::Ui,
    time_area_painter: &egui::Painter,
    line_y_range: RangeInclusive<f32>,
    time_type: TimeType,
) {
    let clip_rect = ui.clip_rect();
    let clip_left = clip_rect.left() as f64;
    let clip_right = clip_rect.right() as f64;

    for segment in &time_ranges_ui.segments {
        let mut x_range = segment.x.clone();
        let mut time_range = segment.time;

        // Cull:
        if *x_range.end() < clip_left {
            continue;
        }
        if clip_right < *x_range.start() {
            continue;
        }

        // Clamp segment to the visible portion to save CPU when zoomed in:
        let left_t = egui::emath::inverse_lerp(x_range.clone(), clip_left).unwrap_or(0.5);
        if 0.0 < left_t && left_t < 1.0 {
            x_range = clip_left..=*x_range.end();
            time_range = TimeRangeF::new(time_range.lerp(left_t), time_range.max);
        }
        let right_t = egui::emath::inverse_lerp(x_range.clone(), clip_right).unwrap_or(0.5);
        if 0.0 < right_t && right_t < 1.0 {
            x_range = *x_range.start()..=clip_right;
            time_range = TimeRangeF::new(time_range.min, time_range.lerp(right_t));
        }

        let x_range = (*x_range.start() as f32)..=(*x_range.end() as f32);
        let rect = Rect::from_x_y_ranges(x_range, line_y_range.clone());
        time_area_painter
            .with_clip_rect(rect)
            .extend(paint_time_range_ticks(ui, &rect, time_type, &time_range));
    }
}

fn paint_time_range_ticks(
    ui: &mut egui::Ui,
    rect: &Rect,
    time_type: TimeType,
    time_range: &TimeRangeF,
) -> Vec<Shape> {
    let font_id = egui::TextStyle::Small.resolve(ui.style());

    match time_type {
        TimeType::Time => {
            paint_ticks(
                ui.ctx(),
                ui.visuals().dark_mode,
                &font_id,
                rect,
                &ui.clip_rect(),
                time_range, // ns
                next_grid_tick_magnitude_ns,
                |ns| format_time_compact(Time::from_ns_since_epoch(ns)),
            )
        }
        TimeType::Sequence => {
            fn next_power_of_10(i: i64) -> i64 {
                i * 10
            }
            paint_ticks(
                ui.ctx(),
                ui.visuals().dark_mode,
                &font_id,
                rect,
                &ui.clip_rect(),
                time_range,
                next_power_of_10,
                |seq| format!("#{seq}"),
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_ticks(
    egui_ctx: &egui::Context,
    dark_mode: bool,
    font_id: &egui::FontId,
    canvas: &Rect,
    clip_rect: &Rect,
    time_range: &TimeRangeF,
    next_time_step: fn(i64) -> i64,
    format_tick: fn(i64) -> String,
) -> Vec<egui::Shape> {
    crate::profile_function!();

    let color_from_alpha = |alpha: f32| -> Color32 {
        if dark_mode {
            Rgba::from_white_alpha(alpha * alpha).into()
        } else {
            Rgba::from_black_alpha(alpha).into()
        }
    };

    let x_from_time = |time: i64| -> f32 {
        let t = (TimeReal::from(time) - time_range.min).as_f32()
            / (time_range.max - time_range.min).as_f32();
        lerp(canvas.x_range(), t)
    };

    let visible_rect = clip_rect.intersect(*canvas);
    let mut shapes = vec![];

    if !visible_rect.is_positive() {
        return shapes;
    }

    let width_time = (time_range.max - time_range.min).as_f32();
    let points_per_time = canvas.width() / width_time;
    let minimum_small_line_spacing = 4.0;
    let expected_text_width = 60.0;

    let line_strength_from_spacing = |spacing_time: i64| -> f32 {
        let next_tick_magnitude = next_time_step(spacing_time) / spacing_time;
        remap_clamp(
            spacing_time as f32 * points_per_time,
            minimum_small_line_spacing..=(next_tick_magnitude as f32 * minimum_small_line_spacing),
            0.0..=1.0,
        )
    };

    let text_color_from_spacing = |spacing_time: i64| -> Color32 {
        let alpha = remap_clamp(
            spacing_time as f32 * points_per_time,
            expected_text_width..=(3.0 * expected_text_width),
            0.0..=0.5,
        );
        color_from_alpha(alpha)
    };

    let max_small_lines = canvas.width() / minimum_small_line_spacing;
    let mut small_spacing_time = 1;
    while width_time / (small_spacing_time as f32) > max_small_lines {
        small_spacing_time = next_time_step(small_spacing_time);
    }
    let medium_spacing_time = next_time_step(small_spacing_time);
    let big_spacing_time = next_time_step(medium_spacing_time);

    // We fade in lines as we zoom in:
    let big_line_strength = line_strength_from_spacing(big_spacing_time);
    let medium_line_strength = line_strength_from_spacing(medium_spacing_time);
    let small_line_strength = line_strength_from_spacing(small_spacing_time);

    let big_line_color = color_from_alpha(0.4 * big_line_strength);
    let medium_line_color = color_from_alpha(0.4 * medium_line_strength);
    let small_line_color = color_from_alpha(0.4 * small_line_strength);

    let big_text_color = text_color_from_spacing(big_spacing_time);
    let medium_text_color = text_color_from_spacing(medium_spacing_time);
    let small_text_color = text_color_from_spacing(small_spacing_time);

    let mut current_time =
        time_range.min.floor().as_i64() / small_spacing_time * small_spacing_time;

    while current_time <= time_range.max.ceil().as_i64() {
        let line_x = x_from_time(current_time);

        if visible_rect.min.x <= line_x && line_x <= visible_rect.max.x {
            let medium_line = current_time % medium_spacing_time == 0;
            let big_line = current_time % big_spacing_time == 0;

            let (height_factor, line_color, text_color) = if big_line {
                (medium_line_strength, big_line_color, big_text_color)
            } else if medium_line {
                (small_line_strength, medium_line_color, medium_text_color)
            } else {
                (0.0, small_line_color, small_text_color)
            };

            // Make line higher if it is stronger:
            let line_top = lerp(canvas.y_range(), lerp(0.75..=0.5, height_factor));

            shapes.push(egui::Shape::line_segment(
                [pos2(line_x, line_top), pos2(line_x, canvas.max.y)],
                Stroke::new(1.0, line_color),
            ));

            if text_color != Color32::TRANSPARENT {
                let text = format_tick(current_time);
                let text_x = line_x + 4.0;

                egui_ctx.fonts(|fonts| {
                    shapes.push(egui::Shape::text(
                        fonts,
                        pos2(text_x, lerp(canvas.y_range(), 0.5)),
                        Align2::LEFT_CENTER,
                        &text,
                        font_id.clone(),
                        text_color,
                    ));
                });
            }
        }

        current_time += small_spacing_time;
    }

    shapes
}
