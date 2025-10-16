use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::archetypes;
use re_view_text_document::TextDocumentView;
use re_viewer_context::{RecommendedView, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn run_test_with_origin(test_context: &mut TestContext, origin: &str, snapshot_name: &str) {
    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_id(
            TextDocumentView::identifier(),
            RecommendedView {
                origin: EntityPath::from(origin),
                query_filter: "$origin/**".parse().expect("invalid entity filter"),
            },
            ViewId::hashed_from_str("test-view-id"),
        ))
    });

    run_view_ui_and_save_snapshot(
        test_context,
        view_id,
        &format!("text_view_{snapshot_name}"),
        egui::vec2(300.0, 300.0),
    );
}

#[test]
fn test_text_documents() {
    let mut test_context = TestContext::new_with_view_class::<TextDocumentView>();

    test_context.log_entity("txt/one", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &archetypes::TextDocument::new("one"),
        )
    });
    test_context.log_entity("txt/two", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &archetypes::TextDocument::new("two"),
        )
    });

    run_test_with_origin(&mut test_context, "txt/one", "one");
    run_test_with_origin(&mut test_context, "txt/two", "two");
    run_test_with_origin(&mut test_context, "txt", "both");
    run_test_with_origin(&mut test_context, "", "root");
}

fn run_view_ui_and_save_snapshot(
    test_context: &TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();
    harness.snapshot(name);
}
