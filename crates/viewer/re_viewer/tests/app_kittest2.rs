use egui_kittest::kittest::Queryable as _;

mod viewer_test_utils;

/// Navigates from welcome to settings screen and snapshots it.
#[tokio::test]
async fn settings_screen() {
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
        .type_text("http://localhost:31338");
    harness.run_ok();
    harness.snapshot("temp2");

    harness.get_by_label("Add").click();
    harness.run_ok();

    harness.snapshot("temp3");
}
