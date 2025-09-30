use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_test_context::{TestContext, external::egui_kittest::SnapshotOptions};
use re_test_viewport::TestContextExt as _;
use re_types::archetypes::Pinhole;
use re_types::components::{Color, Radius};
use re_viewer_context::{RecommendedView, ViewClass as _, ViewId};
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

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(&test_context, view_id, egui::vec2(300.0, 300.0));
}

#[allow(clippy::unwrap_used)]
fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: "/world".into(),
                query_filter: "+ $origin/**".parse().unwrap(),
            },
        );

        let view_id = view_blueprint.id;

        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        view_id
    })
}

fn run_view_ui_and_save_snapshot(test_context: &TestContext, view_id: ViewId, size: egui::Vec2) {
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
