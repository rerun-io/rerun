use egui_kittest::kittest::Queryable as _;
use insta::with_settings;
use re_integration_test::{TestServer, load_test_data};
use re_viewer::viewer_test_utils;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

// #[test]
// pub fn integration_test() {
//     let _server = TestServer::spawn();
//     let test_output = load_test_data();

//     insta::assert_snapshot!(test_output);
// }

#[tokio::test]
pub async fn integration_test_2() {
    let _server = TestServer::spawn();
    let test_output = load_test_data();

    let mut harness = viewer_test_utils::viewer_harness();

    harness.get_by_label("Blueprint panel toggle").click();
    harness.run_ok();

    harness.get_by_label("Add…").click();
    harness.run_ok();
    harness.get_by_label_contains("Add Redap server").click();
    harness.run_ok();
    // harness.snapshot("temp1");

    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Host name:")
        .click();
    harness.run_ok();
    harness
        .get_by_role_and_label(egui::accesskit::Role::TextInput, "Host name:")
        .type_text(&format!("rerun+http://localhost:{}", _server.port));
    harness.run_ok();

    // harness.snapshot("temp2");

    harness.get_by_label("Add").click();
    harness.run_ok();

    tokio::time::sleep(Duration::from_secs(2)).await;
    harness.run_ok();

    harness.get_by_label("my_dataset").click();
    harness.run_ok();

    tokio::time::sleep(Duration::from_secs(2)).await;
    harness.run_ok();

    // viewer_test_utils::step_until(
    //     &mut harness,
    //     |harness| {
    //         harness
    //             .query_by_label_contains("new_recording_id")
    //             .is_some()
    //     },
    //     tokio::time::Duration::from_millis(100),
    //     tokio::time::Duration::from_secs(5),
    // )
    // .await;
    harness.snapshot("temp3");

    insta::assert_snapshot!(test_output);
}
