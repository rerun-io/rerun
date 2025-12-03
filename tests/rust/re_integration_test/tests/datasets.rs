use std::time::Duration;

use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_integration_test::{HarnessExt as _, TestServer};
use re_viewer::viewer_test_utils::{self, HarnessOptions};

#[tokio::test(flavor = "multi_thread")]
pub async fn dataset_ui_test() {
    let server = TestServer::spawn().await.with_test_data().await;

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    let mut snapshot_results = SnapshotResults::new();

    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    harness.get_by_label("Add…").click();
    harness.run_ok();
    harness.get_by_label_contains("Add Redap server").click();
    harness.run_ok();

    snapshot_results.add(harness.try_snapshot("dataset_ui_empty_form"));

    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "URL:")
        .click();
    harness.run_ok();
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "URL:")
        .type_text(&format!("rerun+http://localhost:{}", server.port()));
    harness.run_ok();

    harness.get_by_label("Add").click();
    harness.run_ok();

    viewer_test_utils::step_until(
        "Redap server dataset appears",
        &mut harness,
        // The label eventually appears twice: first in the left panel, and in the entries table
        // when it refreshes. Here we wait for both to appear. Later we pick the first one (in the
        // left panel).
        |harness| harness.query_all_by_label_contains("my_dataset").count() == 2,
        Duration::from_millis(100),
        Duration::from_secs(5),
    );

    // We pick the first one.
    harness
        .get_all_by_label("my_dataset")
        .next()
        .unwrap()
        .click();

    viewer_test_utils::step_until(
        "Redap recording id appears",
        &mut harness,
        |harness| {
            harness
                .query_by_label_contains("new_recording_id")
                .is_some()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    snapshot_results.add(harness.try_snapshot("dataset_ui_table"));
}

#[tokio::test(flavor = "multi_thread")]
pub async fn start_with_dataset_url() {
    let server = TestServer::spawn().await.with_test_data().await;

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(format!(
            "rerun+http://localhost:{}/entry/187b552b95a5c2f73f37894708825ba5",
            server.port()
        )),
        ..Default::default()
    });

    viewer_test_utils::step_until(
        "Redap recording id appears",
        &mut harness,
        |harness| {
            harness
                .query_by_label_contains("new_recording_id")
                .is_some()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    harness.snapshot("start_with_dataset_url");
}
