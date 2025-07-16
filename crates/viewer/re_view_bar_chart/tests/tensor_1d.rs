use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_types::archetypes;
use re_view_bar_chart::BarChartView;
use re_viewer_context::{ViewClass as _, ViewId, test_context::TestContext};
use re_viewport::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::ViewBlueprint;

fn x() -> Vec<f32> {
    (0..100).map(|i| i as f32 * 100.0 / 99.0).collect()
}

#[test]
fn test_bar_chart() {
    let mut test_context = TestContext::new_with_view_class::<BarChartView>();

    test_context.log_entity("tensor", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &archetypes::BarChart::new(x()),
        )
    });

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "tensor_1d",
        egui::vec2(400.0, 300.0),
    );
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            BarChartView::identifier(),
        ))
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            test_context.run_with_single_view(ctx, view_id);
        });
    harness.run();
    harness.snapshot(name);
}
