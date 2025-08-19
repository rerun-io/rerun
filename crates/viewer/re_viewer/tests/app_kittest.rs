use egui_kittest::kittest::Queryable;

mod viewer_test_utils;

/// Navigates from welcome to settings screen and snapshots it.
#[tokio::test]
async fn settings_screen() {
    let mut harness = viewer_test_utils::viewer_harness();
    harness.get_by_label("menu").click();
    harness.run_ok();
    harness.get_by_label_contains("Settings…").click();
    // Wait for the FFmpeg-check loading spinner to disappear.
    viewer_test_utils::step_until(
        &mut harness,
        |harness| {
            harness
                .query_by_label_contains(
                    "The specified FFmpeg binary path does not exist or is not a file.",
                )
                .is_some()
        },
        tokio::time::Duration::from_millis(100),
        tokio::time::Duration::from_secs(5),
    )
    .await;
    harness.snapshot("settings_screen");
}
