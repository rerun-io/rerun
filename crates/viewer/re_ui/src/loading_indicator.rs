//! The loading indicator are three dots that fade in and out in sequence.
//!
//! Similar to this:
//! ⚫️ ⚫️ ⚫️  ↓
//! ⚪️ ⚫️ ⚫️  t
//! ⚪️ ⚪️ ⚫️  i
//! ⚫️ ⚪️ ⚫️  m
//! ⚫️ ⚪️ ⚪️  e
//! ⚫️ ⚫️ ⚪️  ↓
//! ⚫️ ⚫️ ⚫️  ↓

use egui::{Align2, Color32, NumExt as _, Rect, Vec2};

const NUM_DOTS: i32 = 3;

// Let's use `r` (dot radius) as our unit of measurement
// in order to figure out the aspect ratio:
const OUTSIDE_PADDING_IN_R: f32 = 1.0;
const BETWEEN_PADDING_IN_R: f32 = 1.5;
const HEIGHT_IN_R: f32 = OUTSIDE_PADDING_IN_R + 2.0 + OUTSIDE_PADDING_IN_R;
const WIDTH_IN_R: f32 = OUTSIDE_PADDING_IN_R
    + NUM_DOTS as f32 * 2.0
    + (NUM_DOTS as f32 - 1.0) * BETWEEN_PADDING_IN_R
    + OUTSIDE_PADDING_IN_R;

/// We may go below this if pressed for space, but never above.
const DEFAULT_DOT_RADIUS: f32 = 3.0;

/// A loading indicator widget.
///
/// `reason` describes why we are loading. In debug builds, it is shown on hover.
#[doc(alias = "spinner")]
pub fn loading_indicator_ui(ui: &mut egui::Ui, reason: &str) -> egui::Response {
    let r = calc_radius(ui.available_size_before_wrap());
    let size = r * Vec2::new(WIDTH_IN_R, HEIGHT_IN_R);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let opacity = 1.0;
    paint_loading_indicator_inside(ui, Align2::CENTER_CENTER, rect, opacity, None, reason);
    response
}

/// A loading indicator as tall as one line of text, so a label can take its place without
/// changing layout.
///
/// `reason` describes why we are loading. In debug builds, it is shown on hover.
#[doc(alias = "spinner")]
pub fn inline_loading_indicator_ui(ui: &mut egui::Ui, reason: &str) -> egui::Response {
    let line_height = ui.text_style_height(&egui::TextStyle::Body);
    let r = calc_radius(Vec2::new(ui.available_size_before_wrap().x, line_height));
    let size = Vec2::new(r * WIDTH_IN_R, line_height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::ProgressIndicator));
    paint_loading_indicator_inside(ui, Align2::LEFT_CENTER, rect, 1.0, None, reason);
    response
}

pub fn calc_radius(available_space: Vec2) -> f32 {
    let loading_aspect_ratio = WIDTH_IN_R / HEIGHT_IN_R;
    let rect_aspect_ratio = available_space.x / available_space.y;

    // What is the radius of the dots in ui points "pts"?
    if rect_aspect_ratio < loading_aspect_ratio {
        // The loading indicator is wider than the rect, so we are limited by the width of the rect.
        (available_space.x / WIDTH_IN_R).at_most(DEFAULT_DOT_RADIUS)
    } else {
        // The loading indicator is taller than the rect, so we are limited by the height of the rect.
        (available_space.y / HEIGHT_IN_R).at_most(DEFAULT_DOT_RADIUS)
    }
}

/// Paint a reasonably sized loading indicator in the given rectangle, anchored at the given pivot point.
///
/// If `color` is `None`, the spinner uses `visuals.strong_text_color()`.
///
/// `reason` describes why we are loading. In debug builds, it is shown on hover.
///
/// The indicator reports itself to `AccessKit` as a `ProgressIndicator`, so tests can wait for all
/// loading to finish.
#[doc(alias = "spinner")]
pub fn paint_loading_indicator_inside(
    ui: &mut egui::Ui,
    anchor: Align2,
    container_rect: Rect,
    opacity: f32,
    color: Option<Color32>,
    #[cfg_attr(not(debug_assertions), expect(unused_variables))] reason: &str,
) {
    if opacity <= 0.0 {
        return;
    }

    re_tracing::profile_function!();

    let r_pts = calc_radius(container_rect.size());

    if r_pts <= 0.0 || !r_pts.is_finite() {
        return;
    }

    let size_pts = r_pts * Vec2::new(WIDTH_IN_R, HEIGHT_IN_R);
    let rect_pts = anchor.align_size_within_rect(size_pts, container_rect);
    let outside_padding_pts = r_pts * OUTSIDE_PADDING_IN_R;
    let between_padding_pts = r_pts * BETWEEN_PADDING_IN_R;

    let on_color = color.unwrap_or_else(|| ui.visuals().strong_text_color());

    let time = ui.input(|i| i.time);
    let animation_speed = 0.5;

    for i in 0..NUM_DOTS {
        let phase_shift = i as f64 / (NUM_DOTS as f64 + 1.0);
        let phase = (animation_speed * time + 1.0 - phase_shift).fract() as f32;
        // bounce the phase:
        let phase = if phase < 0.5 {
            phase * 2.0
        } else {
            (1.0 - phase) * 2.0
        };
        let alpha = if phase < 0.5 { 2.0 * phase } else { 1.0 };

        let color = on_color.linear_multiply(opacity * alpha);
        let center = rect_pts.left_top()
            + Vec2::new(
                outside_padding_pts + r_pts + i as f32 * (2.0 * r_pts + between_padding_pts),
                outside_padding_pts + r_pts,
            );
        ui.painter().circle_filled(center, r_pts, color);
    }

    // Tell tests and screen readers that something is loading here
    let response = ui.allocate_rect(rect_pts, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::ProgressIndicator));

    #[cfg(debug_assertions)]
    {
        response.on_hover_ui(|ui| {
            crate::UiExt::debug_only_badge(ui);
            ui.label(reason);
        });
    }

    ui.request_repaint();
}
