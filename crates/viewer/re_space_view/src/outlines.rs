use egui::NumExt as _;
use re_ui::ContextExt as _;

// TODO(andreas): It would be nice if these wouldn't need to be set on every single line/point builder.

/// Gap between lines and their outline.
pub const SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES: f32 = 1.0;

/// Gap between points and their outline.
pub const SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES: f32 = 2.5;

/// Produce an [`re_renderer::OutlineConfig`] based on the [`egui::Style`] of the provided [`egui::Context`].
pub fn outline_config(gui_ctx: &egui::Context) -> re_renderer::OutlineConfig {
    // Use the exact same colors we have in the ui!
    let hover_outline = gui_ctx.hover_stroke();
    let selection_outline = gui_ctx.selection_stroke();

    // See also: SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES

    let outline_radius_ui_pts = 0.5 * f32::max(hover_outline.width, selection_outline.width);
    let outline_radius_pixel = (gui_ctx.pixels_per_point() * outline_radius_ui_pts).at_least(0.5);

    re_renderer::OutlineConfig {
        outline_radius_pixel,
        color_layer_a: re_renderer::Rgba::from(hover_outline.color),
        color_layer_b: re_renderer::Rgba::from(selection_outline.color),
    }
}
