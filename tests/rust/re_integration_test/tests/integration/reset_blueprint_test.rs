use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_entity_db::EntityDb;
use re_integration_test::{HarnessExt as _, ViewerHarnessExt as _};
use re_viewer::viewer_test_utils::{self, HarnessOptions};

const RESET_BUTTON: &str = "Reset current blueprint";

fn active_blueprint<'a>(harness: &'a egui_kittest::Harness<'_, re_viewer::App>) -> &'a EntityDb {
    let app = harness.state();
    app.store_hub()
        .active_blueprint_for_route(app.route())
        .expect("active blueprint exists")
}

/// Whether the reset button in the blueprint panel is disabled.
fn reset_button_is_disabled(harness: &egui_kittest::Harness<'_, re_viewer::App>) -> bool {
    harness
        .get_by_label(RESET_BUTTON)
        .accesskit_node()
        .is_disabled()
}

/// The active blueprint is a clone without edits, so the reset button in the blueprint panel is
/// disabled. The user edits the blueprint and clicks the reset button. A new clone of the same
/// blueprint becomes active, and the reset button is disabled again.
#[tokio::test(flavor = "multi_thread")]
pub async fn reset_button_discards_blueprint_edits() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();

    // The viewer writes to the empty blueprint from `init_recording` on its first frames, so that
    // clone counts as edited. A clone of the blueprint after those writes has no edits.
    let source_id = active_blueprint(&harness).store_id().clone();
    harness
        .state_mut()
        .store_hub_mut()
        .set_cloned_blueprint_active_for_app(&source_id)
        .expect("blueprint is cloned");
    harness.run_ok();
    assert!(
        reset_button_is_disabled(&harness),
        "the reset button should be disabled for a blueprint without edits"
    );

    harness.add_blueprint_container(egui_tiles::ContainerKind::Horizontal, None);
    let edited_id = active_blueprint(&harness).store_id().clone();
    assert!(
        !reset_button_is_disabled(&harness),
        "the reset button should be enabled for an edited blueprint"
    );

    harness.click_label(RESET_BUTTON);
    harness.step_until("a new clone is active", |harness| {
        active_blueprint(harness).store_id() != &edited_id
    });

    assert_eq!(
        active_blueprint(&harness).cloned_from(),
        Some(&source_id),
        "the new active blueprint should be a clone of the same blueprint"
    );
    assert!(
        reset_button_is_disabled(&harness),
        "the reset button should be disabled once the edits are discarded"
    );
}
