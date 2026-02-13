//! Test child-parent transform relations all logged at the same entity
//! and time stamp.

use re_log_types::{EntityPath, TimePoint, Timeline};
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_sdk_types::components::Position3D;
use re_sdk_types::datatypes::Angle;
use re_sdk_types::{archetypes, components};
use re_test_context::TestContext;
use re_test_context::VisualizerBlueprintContext as _;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{BlueprintContext as _, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

fn log_transform_tree(test_context: &mut TestContext, time: &TimePoint) {
    test_context.log_entity("transform_tree", |builder| {
        builder
            .with_archetype_auto_row(
                time.clone(),
                &archetypes::Transform3D::default()
                    .with_translation([1.0, 0.0, 0.0])
                    .with_child_frame("shoulder")
                    .with_parent_frame("tf#/"),
            )
            .with_archetype_auto_row(
                time.clone(),
                &archetypes::Transform3D::default()
                    .with_translation([1.0, 0.0, 0.0])
                    .with_child_frame("elbow")
                    .with_parent_frame("shoulder"),
            )
            .with_archetype_auto_row(
                time.clone(),
                &archetypes::Transform3D::default()
                    .with_translation([0.0, 1.0, 0.0])
                    .with_rotation_axis_angle(components::RotationAxisAngle::new(
                        [0.0, 0.0, 1.0],
                        Angle::from_radians(std::f32::consts::PI / 8.0),
                    ))
                    .with_child_frame("hand")
                    .with_parent_frame("elbow"),
            )
    });
}

fn setup_camera(ctx: &re_viewer_context::ViewerContext<'_>, view_id: ViewId) {
    let property = ViewProperty::from_archetype::<EyeControls3D>(
        ctx.blueprint_db(),
        ctx.blueprint_query(),
        view_id,
    );
    property.save_blueprint_component(
        ctx,
        &EyeControls3D::descriptor_position(),
        &Position3D::new(0.0, 0.0, 5.0),
    );
    property.save_blueprint_component(
        ctx,
        &EyeControls3D::descriptor_look_target(),
        &Position3D::new(0.0, 0.0, 0.0),
    );
}

/// Tests correct display of transform axes for transformations that have a set child frame.
#[test]
fn test_transform_tree_visualization() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    // Everything on the same timestamp!
    let timeline = Timeline::new_sequence("time");
    let time = TimePoint::from([(timeline, 0)]);

    log_transform_tree(&mut test_context, &time);

    test_context.send_time_commands(
        test_context.active_store_id(),
        [
            TimeControlCommand::SetActiveTimeline(*timeline.name()),
            TimeControlCommand::SetTime(0.into()),
        ],
    );

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_blueprint =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());

        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);
        setup_camera(ctx, view_id);

        // Override (set) the `TransformAxes3DVisualizer`
        ctx.save_visualizers(
            &EntityPath::from("transform_tree"),
            view_id,
            [&archetypes::TransformAxes3D::new(0.4).with_show_frame(true)],
        );

        view_id
    });

    let mut test_harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(640.0, 480.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    test_harness.run();
    test_harness.snapshot("transform_tree_visualization");
}
