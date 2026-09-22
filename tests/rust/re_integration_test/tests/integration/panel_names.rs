use egui::Role;
use egui_kittest::kittest::Queryable as _;
use re_integration_test::HarnessExt as _;
use re_integration_test::ViewerHarnessExt as _;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

/// The main regions of the viewer are named in the accessibility tree.
///
/// Screen readers and agents driving the viewer over MCP navigate by these names, so an unnamed
/// region is an invisible one.
#[tokio::test(flavor = "multi_thread")]
pub async fn test_panels_are_named() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(true);
    harness.set_time_panel_opened(true);
    harness.run();

    for name in [
        "Top bar",
        "Blueprint panel",
        "Sources panel",
        "Viewport",
        "Selection panel",
        "Time panel",
    ] {
        harness.get_by_role_and_label(Role::Pane, name);
    }
}
