use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint};
use re_sdk_types::archetypes;
use re_sdk_types::encodings::TensorBuffer;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::{SnapshotResults, kittest::Queryable as _};
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
    archetypes::Tensor::new(re_sdk_types::encodings::TensorData::new(
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

#[test]
fn test_tensor_hover() {
    let mut test_context = TestContext::new_with_view_class::<TensorView>();

    test_context.log_entity("tensor", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::STATIC, &make_test_tensor_2d(16))
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_id(
            TensorView::identifier(),
            RecommendedView::new_single_entity("tensor"),
            ViewId::hashed_from_str("test-view-id"),
        ))
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_ui(egui::vec2(256.0, 256.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    // Each texel is 16x16 points, so this is the center of row 2, column 5.
    harness.hover_at(egui::pos2(5.5 * 16.0, 2.5 * 16.0));
    harness.run();

    assert!(harness.query_by_label("2, 5").is_some());
    // The xor test tensor stores `(row ^ column) * 16`.
    assert!(harness.query_by_label("112").is_some());

    harness.snapshot("tensor_2d_hover");
}

#[test]
fn test_tensor_zoom_hover() {
    let mut test_context = TestContext::new_with_view_class::<TensorView>();

    test_context.log_entity("tensor", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::STATIC, &make_test_tensor_2d(16))
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_id(
            TensorView::identifier(),
            RecommendedView::new_single_entity("tensor"),
            ViewId::hashed_from_str("test-view-id"),
        ))
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_ui(egui::vec2(256.0, 256.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    // Zooming 2x around the center shows elements 4 to 12 on both axes.
    harness.hover_at(egui::pos2(128.0, 128.0));
    harness.event(egui::Event::Zoom(2.0));
    harness.run();

    // Each texel is now 32x32 points, so this is the center of row 6, column 9.
    harness.hover_at(egui::pos2(5.5 * 32.0, 2.5 * 32.0));
    harness.run();

    assert!(
        harness.query_by_label("6, 9").is_some(),
        "hover tooltip should show the index of the zoomed-in element"
    );
    // The xor test tensor stores `(row ^ column) * 16`.
    assert!(
        harness.query_by_label("240").is_some(),
        "hover tooltip should show the value of the zoomed-in element"
    );

    harness.snapshot("tensor_2d_zoom_hover");
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
