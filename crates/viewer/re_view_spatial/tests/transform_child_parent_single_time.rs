//! Test child-parent transform relations all logged at the same time stamp.

use re_log_types::{EntityPath, TimePoint, Timeline};
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_sdk_types::components::Position3D;
use re_sdk_types::datatypes::Angle;
use re_sdk_types::{archetypes, blueprint, components};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{BlueprintContext as _, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewContents, ViewProperty};

fn log_transforms(test_context: &mut TestContext, time: &TimePoint) {
    // Log transforms with frame relations
    test_context.log_entity("all_the_transforms", |builder| {
        builder
            .with_archetype_auto_row(
                time.clone(),
                &archetypes::Transform3D::default()
                    .with_scale(0.5)
                    .with_child_frame("red_frame")
                    .with_parent_frame("tf#/"),
            )
            .with_archetype_auto_row(
                time.clone(),
                &archetypes::Transform3D::default()
                    .with_scale(2.0)
                    .with_translation([3.0, 0.0, 0.0])
                    .with_child_frame("green_frame")
                    .with_parent_frame("tf#/"),
            )
            .with_archetype_auto_row(
                time.clone(),
                &archetypes::Transform3D::default()
                    .with_translation([0.0, 3.0, 0.0])
                    .with_rotation_axis_angle(components::RotationAxisAngle::new(
                        [0.0, 0.0, 1.0],
                        Angle::from_radians(std::f32::consts::PI / 4.0),
                    ))
                    .with_child_frame("blue_frame")
                    .with_parent_frame("tf#/"),
            )
    });
}

fn log_boxes(test_context: &mut TestContext, time: &TimePoint) {
    for (name, color) in [
        ("red", components::Color::from_rgb(255, 0, 0)),
        ("green", components::Color::from_rgb(0, 255, 0)),
        ("blue", components::Color::from_rgb(0, 0, 255)),
    ] {
        let half_size = 1.0;
        test_context.log_entity(name, |builder| {
            builder
                .with_archetype_auto_row(
                    time.clone(),
                    &archetypes::Boxes3D::from_half_sizes([(half_size, half_size, half_size)])
                        .with_colors([color])
                        .with_fill_mode(components::FillMode::Solid),
                )
                .with_archetype_auto_row(
                    time.clone(),
                    &archetypes::TransformAxes3D::new(half_size * 2.2).with_show_frame(true),
                )
        });
    }
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
        &Position3D::new(0.0, 0.0, 10.0),
    );
    property.save_blueprint_component(
        ctx,
        &EyeControls3D::descriptor_look_target(),
        &Position3D::new(0.0, 0.0, 0.0),
    );
}

/// Tests correct handling of multiple frame based transforms on the same time stamp, everything in the store.
#[test]
fn test_transform_many_child_parent_relations_on_single_time_and_entity() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    // Everything on the same timestamp!
    let timeline = Timeline::new_sequence("time");
    let time = TimePoint::from([(timeline, 0)]);

    log_boxes(&mut test_context, &time);
    log_transforms(&mut test_context, &time);

    // In this test, the coordinate frames for all boxes are logged in the datastore.
    for name in ["red", "green", "blue"] {
        test_context.log_entity(name, |builder| {
            builder.with_archetype_auto_row(
                time.clone(),
                &archetypes::CoordinateFrame::new(format!("{name}_frame")),
            )
        });
    }

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

        view_id
    });

    let mut test_harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(300.0, 300.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    test_harness.run();
    test_harness.snapshot("transform_many_child_parent_relations_on_single_time_and_entity");
}

/// Tests correct handling of multiple frame based transforms on the same time stamp, using overrides for some of the coordinate frames.
#[test]
fn test_transform_many_child_parent_relations_on_single_time_and_entity_with_coordinate_frame_overrides()
 {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    // Everything on the same timestamp!
    let timeline = Timeline::new_sequence("time");
    let time = TimePoint::from([(timeline, 0)]);

    log_boxes(&mut test_context, &time);
    log_transforms(&mut test_context, &time);

    // Different handling for each coordinate frame:
    // * red: no override
    // * gree: nonsense-value in store, override in blueprint
    // * blue: no value in store, override in blueprint
    test_context.log_entity("red", |builder| {
        builder
            .with_archetype_auto_row(time.clone(), &archetypes::CoordinateFrame::new("red_frame"))
    });
    test_context.log_entity("green", |builder| {
        builder.with_archetype_auto_row(
            time.clone(),
            &archetypes::CoordinateFrame::new("this should never show up"),
        )
    });

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

        // Override green and blue frames:
        ctx.save_blueprint_archetype(
            ViewContents::override_path_for_entity(view_id, &"green".into()),
            &archetypes::CoordinateFrame::new("green_frame"),
        );
        ctx.save_blueprint_archetype(
            ViewContents::override_path_for_entity(view_id, &"blue".into()),
            &archetypes::CoordinateFrame::new("blue_frame"),
        );

        view_id
    });

    let mut test_harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(300.0, 300.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    test_harness.run();
    test_harness.snapshot("transform_many_child_parent_relations_on_single_time_and_entity_with_coordinate_frame_overrides");
}

/// Tests correct display of transform axes for transformations that have a set child frame.
#[test]
fn test_transform_axes_for_explicit_transforms() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    // Everything on the same timestamp!
    let timeline = Timeline::new_sequence("time");
    let time = TimePoint::from([(timeline, 0)]);

    log_transforms(&mut test_context, &time);

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
        let transforms_override_path = ViewContents::override_path_for_entity(
            view_id,
            &EntityPath::from("all_the_transforms"),
        );
        ctx.save_blueprint_archetype(
            transforms_override_path.clone(),
            &blueprint::archetypes::VisualizerOverrides::new([
                // TODO(RR-3153): remove the `as_str()`.
                archetypes::TransformAxes3D::visualizer().as_str(),
            ]),
        );
        ctx.save_blueprint_archetype(
            transforms_override_path,
            &archetypes::TransformAxes3D::new(1.0).with_show_frame(true),
        );

        view_id
    });

    let mut test_harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(300.0, 300.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    test_harness.run();
    test_harness.snapshot("transform_axes_for_explicit_transforms");
}
