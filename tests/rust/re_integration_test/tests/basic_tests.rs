use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_view_text_document::TextDocumentView;
use re_viewer::external::re_types;
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::viewer_test_utils;
use re_viewport_blueprint::ViewBlueprint;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_single_text_document() {
    let mut harness = viewer_test_utils::viewer_harness();
    harness.init_recording();
    harness.toggle_selection_panel();
    harness.snapshot_app("single_text_document_1");

    // Log some data
    harness.log_entity("txt/hello", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_types::archetypes::TextDocument::new("Hello World!"),
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
