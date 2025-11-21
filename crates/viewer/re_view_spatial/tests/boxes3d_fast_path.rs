use std::panic;

use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::{blueprint::archetypes::EyeControls3D, components::Position3D};
use re_viewer_context::{BlueprintContext as _, RecommendedView, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// Test that the fast path renderer works for solid boxes with translation-only transforms.
/// The fast path uses GPU instanced rendering and is automatically selected when:
/// - Fill mode is Solid (not wireframe)
/// - No per-instance rotations
/// - All transforms are translation-only (no rotation/scaling)
#[test]
pub fn test_boxes3d_fast_path() {
    const ADAPTER_ERR: &str = "No graphics adapter found!";

    if let Err(err) = panic::catch_unwind(run_fast_path_test) {
        let mut skip = false;

        if let Some(msg) = err.downcast_ref::<&str>() {
            skip = msg.contains(ADAPTER_ERR);
        } else if let Some(msg) = err.downcast_ref::<String>() {
            skip = msg.contains(ADAPTER_ERR);
        }

        if skip {
            eprintln!("Skipping Boxes3D fast path test: {ADAPTER_ERR}");
            return;
        }

        panic::resume_unwind(err);
    }
}

fn run_fast_path_test() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    // Create 1000 solid boxes with translation-only transforms.
    // This should use the fast instanced rendering path.
    let num_boxes = 1000;
    let mut centers = Vec::with_capacity(num_boxes);
    let mut half_sizes = Vec::with_capacity(num_boxes);
    let mut colors = Vec::with_capacity(num_boxes);

    for i in 0..num_boxes {
        let x = (i % 10) as f32 * 2.5;
        let y = ((i / 10) % 10) as f32 * 2.5;
        let z = (i / 100) as f32 * 2.5;
        centers.push((x, y, z));
        half_sizes.push((1.0, 1.0, 1.0));

        // Cycle through colors for visual variety
        let color = match i % 3 {
            0 => 0xFF0000FF, // Red
            1 => 0x00FF00FF, // Green
            _ => 0x0000FFFF, // Blue
        };
        colors.push(color);
    }

    test_context.log_entity("boxes/fast_path", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &re_types::archetypes::Boxes3D::from_centers_and_half_sizes(centers, half_sizes)
                .with_colors(colors),
        )
    });

    // Setup blueprint and render
    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: "/boxes".into(),
                query_filter: "+ $origin/**".parse().unwrap(),
            },
        );
        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);
        view_id
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::Vec2::new(800.0, 600.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    // Set camera position
    test_context.with_blueprint_ctx(|ctx, _| {
        ViewProperty::from_archetype::<EyeControls3D>(
            ctx.current_blueprint(),
            ctx.blueprint_query(),
            view_id,
        )
        .save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(150.0, 150.0, 150.0),
        );
    });

    harness.run_steps(10);
    harness.snapshot("boxes3d_fast_path");
}
