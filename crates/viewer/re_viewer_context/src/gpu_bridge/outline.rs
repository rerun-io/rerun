//! Utilities related to outlines.

use egui::NumExt as _;

use re_renderer::OutlineConfig;
use re_ui::ContextExt;

/// Produce an [`OutlineConfig`] based on the [`egui::Style`] of the provided [`egui::Context`].
pub fn outline_config(gui_ctx: &egui::Context) -> OutlineConfig {
    // Use the exact same colors we have in the ui!
    let hover_outline = gui_ctx.hover_stroke();
    let selection_outline = gui_ctx.selection_stroke();

    // See also: SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES

    let outline_radius_ui_pts = 0.5 * f32::max(hover_outline.width, selection_outline.width);
    let outline_radius_pixel = (gui_ctx.pixels_per_point() * outline_radius_ui_pts).at_least(0.5);

    OutlineConfig {
        outline_radius_pixel,
        color_layer_a: re_renderer::Rgba::from(hover_outline.color),
        color_layer_b: re_renderer::Rgba::from(selection_outline.color),
    }
}
