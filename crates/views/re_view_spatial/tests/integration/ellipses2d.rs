//! [`Ellipses2D`] tessellation adapts to the size the outline has on screen.

use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::archetypes::Ellipses2D;
use re_sdk_types::blueprint::archetypes::VisualBounds2D;
use re_sdk_types::components::Radius;
use re_sdk_types::{Archetype as _, encodings::Range2D};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::{SpatialView2D, SpatialViewState};
use re_viewer_context::{BlueprintContext as _, ViewClass as _, ViewId, ViewStateExt as _};
use re_viewport_blueprint::ViewBlueprint;

const SNAPSHOT_SIZE: egui::Vec2 = egui::vec2(400.0, 300.0);

/// A circle of radius 100, with the view showing `bounds`.
fn setup(bounds: Range2D) -> (TestContext, ViewId) {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();

    test_context.log_entity("circle", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Ellipses2D::from_half_sizes([[100.0, 100.0]])
                .with_line_radii([Radius::new_ui_points(2.0)]),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(SpatialView2D::identifier());
        let property_path = re_viewport_blueprint::entity_path_for_view_property(
            view.id,
            ctx.store_context
                .blueprint
                .storage_engine()
                .store()
                .entity_tree(),
            VisualBounds2D::name(),
        );
        ctx.save_blueprint_archetype(property_path, &VisualBounds2D::new(bounds));
        blueprint.add_view_at_root(view)
    });

    (test_context, view_id)
}

/// Runs the view until it stops repainting, and returns what it stored for the visualizers.
fn run_and_get_state(test_context: &TestContext, view_id: ViewId) -> SpatialViewState {
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(SNAPSHOT_SIZE)
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));
    harness.run();

    let view_states = test_context.view_states.lock();
    view_states
        .get(&test_context.recording_store_id, view_id)
        .expect("the view ran")
        .downcast_ref::<SpatialViewState>()
        .expect("SpatialView2D should use SpatialViewState")
        .clone()
}

/// Zoomed in on the right edge of the circle, where a fixed 32-gon would show its corners.
#[test]
fn test_ellipses2d_zoomed_in() {
    let (test_context, view_id) = setup(Range2D {
        x_range: [80.0, 110.0].into(),
        y_range: [-15.0, 15.0].into(),
    });

    test_context
        .run_view_ui_and_save_renderer_snapshot(
            view_id,
            "ellipses2d_zoomed_in",
            SNAPSHOT_SIZE,
            None,
        )
        .unwrap();
}

#[test]
fn test_ellipses2d_visualizers_see_the_view() {
    let (test_context, view_id) = setup(Range2D {
        x_range: [80.0, 110.0].into(),
        y_range: [-15.0, 15.0].into(),
    });

    let screen_space = run_and_get_state(&test_context, view_id)
        .screen_space_2d
        .expect("a 2D view stores its screen space");

    // The bounds fill the view's height, and are widened to its aspect ratio.
    let visible = screen_space.visible_scene_rect;
    assert_eq!((visible.min.y, visible.max.y), (-15.0, 15.0));
    assert!((visible.center().x - 95.0).abs() < 1e-3, "{visible:?}");
    assert!(visible.width() > 30.0, "{visible:?}");
    // About 300 pixels for 30 units, less the space the view's frame takes.
    assert!(
        (9.0..=10.0).contains(&screen_space.pixels_per_scene_unit),
        "{screen_space:?}"
    );
}

/// Zero-size bounds give a NaN scale, which must not reach the visualizers or repaint forever.
#[test]
fn test_ellipses2d_degenerate_bounds_settle() {
    let (test_context, view_id) = setup(Range2D {
        x_range: [0.0, 0.0].into(),
        y_range: [0.0, 0.0].into(),
    });

    assert_eq!(
        run_and_get_state(&test_context, view_id).screen_space_2d,
        None
    );
}
