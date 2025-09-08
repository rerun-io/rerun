#![cfg(feature = "testing")]

use std::time::Duration;

use egui_kittest::kittest::Queryable as _;

use re_viewer::viewer_test_utils;

/// Navigates from welcome to settings screen and snapshots it.
#[tokio::test]
async fn settings_screen() {
    let mut harness = viewer_test_utils::viewer_harness();
    harness.get_by_label("Menu").click();
    harness.run_ok();
    harness.get_by_label_contains("Settings…").click();
    // Wait for the FFmpeg-check loading spinner to disappear.
    viewer_test_utils::step_until(
        "Settings screen shows up with FFMpeg binary not found error",
        &mut harness,
        |harness| {
            harness
                .query_by_label_contains(
                    "The specified FFmpeg binary path does not exist or is not a file.",
                )
                .is_some()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    harness.snapshot("settings_screen");
}
