use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint};
use re_sdk_types::archetypes;
use re_sdk_types::datatypes::TensorBuffer;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_view_tensor::TensorView;
use re_viewer_context::{RecommendedView, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

// Fun little xor texture for testing
fn make_test_tensor_2d(size: usize) -> archetypes::Tensor {
    let scale = (256 / size) as u8;
    let data = (0..size)
        .flat_map(|i| (0..size).map(move |j| (i ^ j) as u8 * scale))
        .collect::<Vec<_>>();
    archetypes::Tensor::new(re_sdk_types::datatypes::TensorData::new(
        vec![size as u64, size as u64],
        TensorBuffer::U8(data.into()),
    ))
}

fn run_test_with_origin(
    test_context: &mut TestContext,
    origin: &str,
    snapshot_name: &str,
    snapshot_results: &mut SnapshotResults,
) {
    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_id(
            TensorView::identifier(),
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
        &format!("tensor_2d_{snapshot_name}"),
        egui::vec2(256.0, 256.0),
        snapshot_results,
    );
}

#[test]
fn test_tensor() {
    let mut test_context = TestContext::new_with_view_class::<TensorView>();

    test_context.log_entity("tensors/t1", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::STATIC, &make_test_tensor_2d(16))
    });
    test_context.log_entity("tensors/t2", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::STATIC, &make_test_tensor_2d(8))
    });

    let mut snapshot_results = SnapshotResults::new();
    run_test_with_origin(&mut test_context, "tensors/t1", "t1", &mut snapshot_results);
    run_test_with_origin(&mut test_context, "tensors/t2", "t2", &mut snapshot_results);
    run_test_with_origin(&mut test_context, "tensors", "both", &mut snapshot_results);
    run_test_with_origin(&mut test_context, "", "root", &mut snapshot_results);
}

fn run_view_ui_and_save_snapshot(
    test_context: &TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
    snapshot_results: &mut SnapshotResults,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    harness.run();
    harness.snapshot(name);
    snapshot_results.extend_harness(&mut harness);
}
