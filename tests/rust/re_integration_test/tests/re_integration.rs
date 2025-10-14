use std::time::Duration;

use egui_kittest::{SnapshotResults, kittest::Queryable as _};

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
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Host name:")
        .click();
    harness.run_ok();
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Host name:")
        .type_text(&format!("rerun+http://localhost:{}", server.port()));
    harness.run_ok();

    harness.get_by_label("Add").click();
    harness.run_ok();

    viewer_test_utils::step_until(
        "Redap server dataset appears",
        &mut harness,
        |harness| harness.query_by_label_contains("my_dataset").is_some(),
        Duration::from_millis(100),
        Duration::from_secs(5),
    );

    harness.get_by_label("my_dataset").click();
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
