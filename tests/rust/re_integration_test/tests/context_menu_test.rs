use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_view_text_document::TextDocumentView;
use re_viewer::external::re_types;
use re_viewer::external::re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

#[tokio::test(flavor = "multi_thread")]
pub async fn test_stream_context_single_select() {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions::default());
    harness.init_recording();
    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(true);

    // Log some data
    harness.log_entity("txt/hello/world", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_types::archetypes::TextDocument::new("Hello World!"),
        )
    });

    // Set up the viewport blueprint
    harness.clear_current_blueprint();

    let text_document_view = ViewBlueprint::new(
        TextDocumentView::identifier(),
        RecommendedView {
            origin: "/txt/hello".into(),
            query_filter: "+ $origin/**".parse().unwrap(),
        },
    );
    harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        blueprint.add_view_at_root(text_document_view);
    });

    // Click streams tree items and check their context menu
    harness.right_click_label("txt/");
    harness.snapshot_app("streams_context_single_select_1");

    harness.click_label("Expand all");
    harness.snapshot_app("streams_context_single_select_2");

    harness.right_click_label("world");
    harness.snapshot_app("streams_context_single_select_3");

    harness.key_press(egui::Key::Escape);
    harness.snapshot_app("streams_context_single_select_4");

    harness.right_click_label("text");
    harness.snapshot_app("streams_context_single_select_5");
}
