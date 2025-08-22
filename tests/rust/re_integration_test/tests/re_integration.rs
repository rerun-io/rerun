use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use insta::with_settings;
use re_integration_test::{TestServer, load_test_data};
use re_viewer::viewer_test_utils;

// #[test] // TODO(emilk): re-enable when they actually work
pub fn integration_test() {
    let server = TestServer::spawn();
    let test_output = load_test_data(server.port());

    with_settings!({
        filters => vec![
            (r"Re-importing rerun from: .+", "Re-importing rerun from: /fake/path"),
        ]
    }, {
        insta::assert_snapshot!(test_output);
    });
}

#[cfg(not(windows)) // TODO(emilk): fix
#[tokio::test]
pub async fn dataset_ui_test() {
    let server = TestServer::spawn();
    load_test_data(server.port());

    let mut harness = viewer_test_utils::viewer_harness();
    let mut snapshot_results = SnapshotResults::new();

    harness.get_by_label("Blueprint panel toggle").click();
    harness.run_ok();

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
        &mut harness,
        |harness| harness.query_by_label_contains("my_dataset").is_some(),
        tokio::time::Duration::from_millis(100),
        tokio::time::Duration::from_secs(5),
    )
    .await;

    harness.get_by_label("my_dataset").click();
    viewer_test_utils::step_until(
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
