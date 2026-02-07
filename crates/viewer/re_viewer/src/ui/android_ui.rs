//! Android-specific UI adaptations for the Rerun Viewer.
//!
//! This module provides touch-friendly style adjustments when running on Android.

/// Apply Android-specific style adjustments to the egui context.
///
/// Call this once during app initialization to configure egui for touch-friendly
/// interaction on Android devices (larger hit targets, wider scrollbars, etc.).
pub fn apply_android_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Increase interaction target sizes for touch
    style.interaction.interact_radius = 12.0; // Default is 5.0
    style.interaction.resize_grab_radius_side = 12.0;
    style.interaction.resize_grab_radius_corner = 16.0;

    // Larger spacing for touch targets
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.item_spacing = egui::vec2(10.0, 6.0);

    // Larger scroll bar for touch
    style.spacing.scroll.bar_width = 12.0;
    style.spacing.scroll.handle_min_length = 32.0;

    ctx.set_style(style);
}
