use egui::Vec2;
use egui_kittest::{HarnessBuilder, SnapshotOptions};

/// What is the purpose of the test?
#[derive(Clone, Copy, Debug)]
pub enum TestOptions {
    /// Pure egui
    Gui,

    /// Some 3D rendering (requires higher thresholds)
    Rendering3D,
}

pub fn new_harness<T>(option: TestOptions, size: impl Into<Vec2>) -> HarnessBuilder<T> {
    re_log::setup_logging(); // It's nice with log output from tests

    let size = size.into();

    let options = match option {
        TestOptions::Gui => default_snapshot_options_for_ui(),
        TestOptions::Rendering3D => default_snapshot_options_for_3d(size),
    };

    egui_kittest::Harness::builder()
        .wgpu()
        .with_size(size)
        .with_options(options)
}

pub fn default_snapshot_options_for_ui() -> SnapshotOptions {
    // TODO(aedm): allow zero on CI and warn users if they generate snapshots on GPU
    SnapshotOptions::default().failed_pixel_count_threshold(
        10, // we sometimes have a few wrong pixels in text rendering in egui for unknown reasons
    )
}

pub fn default_snapshot_options_for_3d(viewport_size: Vec2) -> SnapshotOptions {
    // We sometime have "binary" failures, e.g. a pixel being categorized
    // as either inside or outside a primitive due to platform differences.
    // How many depend on the size of the image.
    let num_total_pixels = viewport_size.x * viewport_size.y;

    let broken_pixels_fraction = 0.04 / 100.0;
    let max_broken_pixels = (num_total_pixels * broken_pixels_fraction).round() as usize;

    // Need a bit higher than the default to accommodate for various filtering artifacts, typically caused by the grid shader.
    let threshold = 1.0;

    SnapshotOptions::default()
        .threshold(threshold)
        .failed_pixel_count_threshold(max_broken_pixels)
}
