use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_view_text_document::TextDocumentView;
use re_viewer::external::re_sdk_types;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_single_text_document() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(true);

    harness.snapshot_app("single_text_document_1");

    // Log some data
    harness.log_entity("txt/hello", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::TextDocument::new("Hello World!"),
        )
    });

    // Set up the viewport blueprint
    harness.clear_current_blueprint();
    harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            TextDocumentView::identifier(),
        ));
    });

    harness.snapshot_app("single_text_document_2");
}
