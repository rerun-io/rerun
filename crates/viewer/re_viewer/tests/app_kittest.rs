#![cfg(feature = "testing")]

use egui_kittest::kittest::Queryable as _;

use re_viewer::viewer_test_utils;
use re_viewer::viewer_test_utils::StepUntil;

/// Navigates from welcome to settings screen and snapshots it.
#[tokio::test]
async fn settings_screen() {
    let mut harness = viewer_test_utils::viewer_harness();
    harness.get_by_label("Menu").click();
    harness.run_ok();
    harness.get_by_label_contains("Settings…").click();
    // Wait for the FFmpeg-check loading spinner to disappear.
    StepUntil::new("Settings screen shows up with FFMpeg binary not found error")
        .run(&mut harness, |harness| {
            harness.query_by_label_contains(
                "The specified FFmpeg binary path does not exist or is not a file.",
            )
        })
        .await;
    harness.snapshot("settings_screen");
}
