use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::archetypes;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_tensor::TensorView;
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn x() -> Vec<f32> {
    (0..100).map(|i| i as f32 * 100.0 / 99.0).collect()
}

#[test]
fn test_tensor() {
    let mut test_context = TestContext::new_with_view_class::<TensorView>();

    test_context.log_entity("tensor", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &archetypes::Tensor::new(x()),
        )
    });

    let view_id = setup_blueprint(&mut test_context);
    test_context
        .run_view_ui_and_save_snapshot(view_id, "tensor_1d", egui::vec2(300.0, 50.0), None)
        .unwrap();
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            TensorView::identifier(),
        ))
    })
}
