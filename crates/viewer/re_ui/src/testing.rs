use egui::Vec2;
use egui_kittest::SnapshotOptions;

pub fn default_snapshot_options_for_ui() -> SnapshotOptions {
    SnapshotOptions::default().failed_pixel_count_threshold(4)
}

pub fn default_snapshot_options_for_3d(viewport_size: Vec2) -> SnapshotOptions {
    // We sometime have "binary" failures, e.g. a pixel being categorized
    // as either inside or outside a primitive due to platform differences.
    // How many depend on the size of the image.
    let num_total_pixels = viewport_size.x * viewport_size.y;

    let broken_pixels_fraction = 1e-4;
    let max_broken_pixels = (num_total_pixels * broken_pixels_fraction).round() as usize;

    let threshold = 0.9; // Slightly higher than the default

    SnapshotOptions::default()
        .threshold(threshold)
        .failed_pixel_count_threshold(max_broken_pixels)
}
