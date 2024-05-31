//! This crate implements various component editors.
//!
//! The only entry point is [`register_editors`], which registers all editors in the component UI registry.
//! This should be called by `re_viewer` on startup.

mod corner2d;
mod marker_shape;
mod response_utils;
mod visible;

// ----

use egui::NumExt as _;
use re_types::components::{Color, MarkerSize, Name, Radius, StrokeWidth, Text};
use re_viewer_context::ViewerContext;

// ----

fn edit_color_ui(_ctx: &ViewerContext<'_>, ui: &mut egui::Ui, value: &mut Color) -> egui::Response {
    let mut edit_color = (*value).into();
    let response = egui::color_picker::color_edit_button_srgba(
        ui,
        &mut edit_color,
        // TODO(#1611): No transparency supported right now.
        // Once we do we probably need to be more explicit about the component semantics.
        egui::color_picker::Alpha::Opaque,
    );
    *value = edit_color.into();
    response
}

fn edit_text_ui(_ctx: &ViewerContext<'_>, ui: &mut egui::Ui, value: &mut Text) -> egui::Response {
    let mut edit_text = value.to_string();
    let response = egui::TextEdit::singleline(&mut edit_text).show(ui).response;
    *value = edit_text.into();
    response
}

fn edit_name_ui(_ctx: &ViewerContext<'_>, ui: &mut egui::Ui, value: &mut Name) -> egui::Response {
    let mut edit_name = value.to_string();
    let response = egui::TextEdit::singleline(&mut edit_name).show(ui).response;
    *value = edit_name.into();
    response
}

fn edit_radius_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut Radius,
) -> egui::Response {
    let speed = (value.0 * 0.01).at_least(0.001);

    ui.add(
        egui::DragValue::new(&mut value.0)
            .clamp_range(0.0..=f64::INFINITY)
            .speed(speed),
    )
}

fn edit_stroke_width_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut StrokeWidth,
) -> egui::Response {
    let speed = (value.0 * 0.01).at_least(0.001);
    ui.add(
        egui::DragValue::new(&mut value.0)
            .clamp_range(0.0..=f64::INFINITY)
            .speed(speed),
    )
}

fn edit_marker_size_ui(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MarkerSize,
) -> egui::Response {
    let speed = (value.0 * 0.01).at_least(0.001);
    ui.add(
        egui::DragValue::new(&mut value.0)
            .clamp_range(0.0..=f64::INFINITY)
            .speed(speed),
    )
}

// ----

/// Registers all editors of this crate in the component UI registry.
///
/// ⚠️ This is supposed to be the only export of this crate.
/// This crate is meant to be a leaf crate in the viewer ecosystem and should only be used by the `re_viewer` crate itself.
pub fn register_editors(registry: &mut re_viewer_context::ComponentUiRegistry) {
    registry.add_editor(edit_color_ui);
    registry.add_editor(corner2d::edit_corner2d);
    registry.add_editor(marker_shape::edit_marker_shape_ui);
    registry.add_editor(edit_marker_size_ui);
    registry.add_editor(edit_name_ui);
    registry.add_editor(edit_radius_ui);
    registry.add_editor(edit_stroke_width_ui);
    registry.add_editor(edit_text_ui);
    registry.add_editor(visible::edit_visible);
}
