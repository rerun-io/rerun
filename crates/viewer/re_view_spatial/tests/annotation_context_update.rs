//! Test that changing an `AnnotationContext` across timeline steps correctly updates
//! the colors of a `Points3D` point cloud (i.e. invalidates the `Points3D` cache).

use re_log_types::{TimeInt, TimePoint, Timeline};
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_sdk_types::components::Position3D;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{BlueprintContext as _, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// Log a point cloud with `class_ids`, and an annotation context that changes color between two frames.
///
/// Frame 1: annotation maps class 0 → red, class 1 → green
/// Frame 2: annotation maps class 0 → blue, class 1 → yellow
///
/// The point cloud itself is static (same positions and `class_ids` on both frames).
/// If the cache correctly accounts for the annotation context, the colors should change.
#[test]
pub fn test_annotation_context_update_on_points3d() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    let timeline = Timeline::new_sequence("frame");
    test_context.set_active_timeline(*timeline.name());

    let frame = |seq: i64| {
        TimePoint::default().with(
            timeline,
            TimeInt::from_sequence(seq.try_into().expect("unexpected min value")),
        )
    };

    // Log a static point cloud with two points using class_ids.
    test_context.log_entity("points", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Points3D::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]])
                .with_radii([0.3])
                .with_class_ids([0, 1]),
        )
    });

    // Frame 1: class 0 → red, class 1 → green
    test_context.log_entity("/", |builder| {
        builder.with_archetype_auto_row(
            frame(1),
            &re_sdk_types::archetypes::AnnotationContext::new([
                (
                    0,
                    "red",
                    re_sdk_types::datatypes::Rgba32::from_rgb(255, 0, 0),
                ),
                (
                    1,
                    "green",
                    re_sdk_types::datatypes::Rgba32::from_rgb(0, 255, 0),
                ),
            ]),
        )
    });

    // Frame 2: class 0 → blue, class 1 → yellow
    test_context.log_entity("/", |builder| {
        builder.with_archetype_auto_row(
            frame(2),
            &re_sdk_types::archetypes::AnnotationContext::new([
                (
                    0,
                    "blue",
                    re_sdk_types::datatypes::Rgba32::from_rgb(0, 0, 255),
                ),
                (
                    1,
                    "yellow",
                    re_sdk_types::datatypes::Rgba32::from_rgb(255, 255, 0),
                ),
            ]),
        )
    });

    let view_id = setup_blueprint(&mut test_context);

    run_view_ui_and_save_snapshot(&test_context, view_id, "annotation_context_update");
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_blueprint =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());

        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        // Set eye position so both points are clearly visible.
        let property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.blueprint_db(),
            ctx.blueprint_query(),
            view_id,
        );
        property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(0.5, 2.0, 3.0),
        );
        property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(0.5, 0.0, 0.0),
        );

        view_id
    })
}

fn run_view_ui_and_save_snapshot(test_context: &TestContext, view_id: ViewId, name: &str) {
    let size = egui::vec2(200.0, 200.0);

    let mut snapshot_results = SnapshotResults::new();
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    // Frame 1: should show red + green points.
    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetTime(1_i64.into())],
    );
    harness.run();
    snapshot_results.add(harness.try_snapshot(format!("{name}_frame1")));

    // Frame 2: should show blue + yellow points (not red + green).
    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetTime(2_i64.into())],
    );
    harness.run();
    snapshot_results.add(harness.try_snapshot(format!("{name}_frame2")));
}
