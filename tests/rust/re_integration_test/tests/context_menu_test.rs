use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_view_text_document::TextDocumentView;
use re_viewer::external::re_sdk_types;
use re_viewer::external::re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
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
            &re_sdk_types::archetypes::TextDocument::new("Hello World!"),
        )
    });
    harness
}

fn setup_single_view_blueprint(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) {
    harness.clear_current_blueprint();

    let text_document_view = ViewBlueprint::new(
        TextDocumentView::identifier(),
        RecommendedView {
            origin: "/txt/hello".into(),
            query_filter: "+ $origin/**"
                .parse()
                .expect("Failed to parse query filter"),
        },
    );
    harness.setup_viewport_blueprint(|_viewer_context, blueprint| {
        blueprint.add_view_at_root(text_document_view);
    });
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_stream_context_single_select() {
    let mut harness = make_test_harness();
    setup_single_view_blueprint(&mut harness);

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

#[tokio::test(flavor = "multi_thread")]
pub async fn test_blueprint_view_context() {
    let mut harness = make_test_harness();
    setup_single_view_blueprint(&mut harness);

    // There are two nodes with that label, the second one is the view widget.
    harness.right_click_nth_label("txt/hello", 1);

    harness.snapshot_app("blueprint_view_context");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_container_selection_context_menu() {
    let mut harness = make_test_harness();

    // Set up the viewport blueprint
    harness.clear_current_blueprint();

    let container_id = harness.add_blueprint_container(egui_tiles::ContainerKind::Vertical, None);

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        let mut view_1 = ViewBlueprint::new_with_root_wildcard(TextDocumentView::identifier());
        view_1.display_name = Some("View 1".into());
        let mut view_2 = ViewBlueprint::new_with_root_wildcard(TextDocumentView::identifier());
        view_2.display_name = Some("View 2".into());
        blueprint.add_views([view_1, view_2].into_iter(), Some(container_id), None);
    });

    harness.click_label("Vertical container");
    harness.set_selection_panel_opened(true);

    // There are multiple nodes with that label, second and third are
    // the ones on the selection panel.
    harness.selection_panel().right_click_label("View 1");
    harness.snapshot_app("container_selection_context_menu_1");

    harness.key_press(egui::Key::Escape);
    harness.selection_panel().right_click_label("View 2");
    harness.snapshot_app("container_selection_context_menu_2");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_collapse_stream_entity() {
    let mut harness = make_test_harness();
    setup_single_view_blueprint(&mut harness);

    harness.streams_tree().right_click_label("txt/");
    harness.snapshot_app("collapse_stream_entity_1");

    harness.click_label("Collapse all");
    harness.snapshot_app("collapse_stream_entity_2");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_collapse_stream_root() {
    let mut harness = make_test_harness();
    setup_single_view_blueprint(&mut harness);
    harness.snapshot_app("collapse_stream_root_1");

    harness.streams_tree().right_click_label("/");
    harness.snapshot_app("collapse_stream_root_2");

    harness.click_label("Collapse all");
    harness.snapshot_app("collapse_stream_root_3");
}
