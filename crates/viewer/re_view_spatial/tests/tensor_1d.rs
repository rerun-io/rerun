use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_types::{archetypes, datatypes};
use re_view_spatial::SpatialView2D;
use re_viewer_context::{ViewClass as _, ViewId, test_context::TestContext};
use re_viewport::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::ViewBlueprint;

fn x() -> Vec<f64> {
    (0..100).map(|i| i as f64 * 100.0 / 99.0).collect()
}

#[test]
fn test_tensor() -> anyhow::Result<()> {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();

    let tensor_data =
        datatypes::TensorData::new(vec![1, 100], datatypes::TensorBuffer::F64(x().into()));
    let image =
        archetypes::Image::from_color_model_and_tensor(datatypes::ColorModel::L, tensor_data)?;
    test_context.log_entity("tensor", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::STATIC, &image)
    });

    test_context.save_recording_to_file("tensor_1d_image.rrd")?;

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "tensor_1d",
        egui::vec2(300.0, 50.0),
    );

    Ok(())
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            SpatialView2D::identifier(),
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
