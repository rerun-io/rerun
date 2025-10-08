use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_test_context::{TestContext, external::egui_kittest::SnapshotOptions};
use re_test_viewport::TestContextExt as _;
use re_types::archetypes::Pinhole;
use re_types::components::{Color, Radius};
use re_viewer_context::{ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_pinhole_camera() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    test_context.log_entity("world/camera", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &Pinhole::from_focal_length_and_resolution([3., 3.], [3., 3.])
                .with_color(Color::from_rgb(255, 144, 1)) // #FF9001
                .with_line_width(Radius::new_ui_points(2.0)),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
        blueprint.add_view_at_root(view)
    });

    run_view_ui_and_save_snapshot(&mut test_context, view_id, egui::vec2(300.0, 300.0));
}

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    view_id: ViewId,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    let raw_input = harness.input_mut();
    // TODO(#6825): use blueprint view setup once we can control camera from blueprints.
    raw_input
        .events
        .push(egui::Event::PointerMoved((100.0, 100.0).into()));
    raw_input.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::Vec2::UP * 2.0,
        modifiers: egui::Modifiers::default(),
    });
    harness.run_steps(10);
    let broken_pixels_fraction = 0.0045;

    harness.snapshot_options(
        "pinhole_camera",
        &SnapshotOptions::new().failed_pixel_count_threshold(
            (size.x * size.y * broken_pixels_fraction).round() as usize,
        ),
    );
}
