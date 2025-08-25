use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_integration_test::TestServer;
use re_viewer::viewer_test_utils;

#[tokio::test(flavor = "multi_thread")]
pub async fn dataset_ui_test() {
    let server = TestServer::spawn().await.with_test_data().await;

    let mut harness = viewer_test_utils::viewer_harness();
    let mut snapshot_results = SnapshotResults::new();

    harness.get_by_label("Blueprint panel toggle").click();
    harness.run_ok();

    harness.get_by_label("Add…").click();
    harness.run_ok();
    harness.get_by_label_contains("Add Redap server").click();
    harness.run_ok();

    // TODO(#10989): re-enable this snapshot when we can work around the welcome screen
    // snapshot_results.add(harness.try_snapshot("dataset_ui_empty_form"));

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
        tokio::time::Duration::from_millis(100),
        tokio::time::Duration::from_secs(5),
    )
    .await;

    harness.get_by_label("my_dataset").click();
    viewer_test_utils::step_until(
        "Redap recording id appears",
        &mut harness,
        |harness| {
            harness
                .query_by_label_contains("new_recording_id")
                .is_some()
        },
        tokio::time::Duration::from_millis(100),
        tokio::time::Duration::from_secs(5),
    )
    .await;
    snapshot_results.add(harness.try_snapshot("dataset_ui_table"));
}
