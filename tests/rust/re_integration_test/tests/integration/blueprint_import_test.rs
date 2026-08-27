//! Importing a blueprint file applies it to the currently open recording, whichever application
//! the blueprint happens to have been saved under.

use std::path::{Path, PathBuf};

use re_integration_test::HarnessExt as _;
use re_log_types::{ApplicationId, StoreId, StoreKind};
use re_sdk::RecordingStreamBuilder;
use re_sdk_types::ViewClassIdentifier;
use re_viewer::App;
use re_viewer::viewer_test_utils::{self, AppTestingExt as _, step_until};
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::ViewBlueprint;

type Harness<'h> = egui_kittest::Harness<'h, App>;

/// The application the blueprint file belongs to. Deliberately not the one importing it.
const SOURCE_APPLICATION: &str = "rerun_example_unrelated_blueprint_application";

/// A dropped file backed by a path on disk. The native egui implementation lives inside `egui-winit`
/// as a private type, so tests have to bring their own.
#[derive(Debug)]
struct DroppedPath(PathBuf);

impl egui::DroppedFile for DroppedPath {
    fn path(&self) -> &Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|err| err.to_string())
    }
}

/// Saves an `.rbl` under [`SOURCE_APPLICATION`], holding a single `BarChartView`.
fn save_source_blueprint(path: &Path) {
    let (rec, storage) = RecordingStreamBuilder::new(SOURCE_APPLICATION)
        .memory()
        .expect("recording stream should be created");

    re_sdk::blueprint::Blueprint::new(re_sdk::blueprint::BarChartView::new("bar chart"))
        .send(&rec, re_sdk::blueprint::BlueprintActivation::default())
        .expect("blueprint should be sent");

    let blueprint_msgs = storage
        .take()
        .into_iter()
        .filter(|msg| msg.store_id().kind() == StoreKind::Blueprint)
        .map(Ok);

    let mut file = std::fs::File::create(path).expect("blueprint file should be created");
    re_log_encoding::Encoder::encode_into(
        re_build_info::CrateVersion::LOCAL,
        re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED,
        blueprint_msgs,
        &mut file,
    )
    .expect("blueprint should encode");
}

fn app_id(harness: &mut Harness<'_>) -> ApplicationId {
    harness
        .state_mut()
        .testonly_get_route()
        .app_id()
        .expect("route should have an app id")
        .clone()
}

fn active_blueprint_id(harness: &mut Harness<'_>) -> Option<StoreId> {
    let app_id = app_id(harness);
    harness
        .state_mut()
        .testonly_get_store_hub()
        .active_blueprint_id_for_app(&app_id)
        .cloned()
}

fn default_blueprint_id(harness: &mut Harness<'_>) -> Option<StoreId> {
    let app_id = app_id(harness);
    harness
        .state_mut()
        .testonly_get_store_hub()
        .default_blueprint_id_for_app(&app_id)
        .cloned()
}

/// The view classes present in the currently active blueprint.
fn active_view_classes(harness: &mut Harness<'_>) -> Vec<ViewClassIdentifier> {
    harness.run_with_viewer_context(|ctx| {
        re_viewport_blueprint::ViewportBlueprint::from_db(ctx.blueprint_db(), ctx.blueprint_query)
            .views
            .values()
            .map(|view| view.class_identifier())
            .collect()
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn test_dropped_blueprint_is_applied_to_open_recording() {
    let rbl_file = tempfile::Builder::new()
        .suffix(".rbl")
        .tempfile()
        .expect("temp file should be created");
    save_source_blueprint(rbl_file.path());

    // A viewer with a recording of a *different* application open.
    let mut harness = viewer_test_utils::viewer_harness(&Default::default());
    harness.init_recording();

    let opened_app_id = app_id(&mut harness);
    assert_ne!(
        opened_app_id.as_str(),
        SOURCE_APPLICATION,
        "the blueprint file must belong to a different application than the one importing it"
    );

    // Give the open application a blueprint of its own, holding a different view, so that the
    // import has something real to displace.
    let view_class = re_view_text_document::TextDocumentView::identifier();
    harness.setup_viewport_blueprint(move |_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(view_class))
    });

    let blueprint_before = active_blueprint_id(&mut harness)
        .expect("the open recording should have an active blueprint");
    assert_eq!(
        active_view_classes(&mut harness),
        vec![view_class],
        "the pre-import blueprint should hold exactly the view we just added"
    );

    // Drop the blueprint file onto the viewer.
    harness
        .input_mut()
        .dropped_files
        .push(std::sync::Arc::new(DroppedPath(rbl_file.path().to_owned())));

    // Importing decodes on a dedicated thread, so poll rather than assuming a step count.
    step_until(
        "the imported blueprint became active",
        &mut harness,
        |harness| active_blueprint_id(harness).is_some_and(|id| id != blueprint_before),
    );

    // The imported blueprint must have been filed under the open application, not the one it was
    // saved under.
    let blueprint_after =
        active_blueprint_id(&mut harness).expect("there should still be an active blueprint");
    assert_eq!(
        blueprint_after.application_id(),
        &opened_app_id,
        "the imported blueprint should have been retargeted onto the open application"
    );

    // …and the active blueprint must be the one we imported, not just any new one.
    assert_eq!(
        active_view_classes(&mut harness),
        vec![re_view_bar_chart::BarChartView::identifier()],
        "the imported blueprint should have replaced the previously active one"
    );

    // The activation command asks to be made both active and default, so the import must also
    // become the application's default blueprint.
    let default_after = default_blueprint_id(&mut harness)
        .expect("the imported blueprint should have become the application's default");
    assert_eq!(
        default_after.application_id(),
        &opened_app_id,
        "the default blueprint should have been retargeted onto the open application too"
    );
}
