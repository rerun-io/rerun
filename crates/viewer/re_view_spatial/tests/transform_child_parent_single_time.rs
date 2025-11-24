use re_log_types::{TimePoint, Timeline};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::{
    archetypes, blueprint::archetypes::EyeControls3D, components, components::Position3D,
    datatypes::Angle,
};
use re_viewer_context::{BlueprintContext as _, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// Tests correct handling of multiple frame based transforms on the same time stamp.
#[test]
fn test_transform_many_child_parent_relations_on_single_time_and_entity() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    // Everything on the same timestamp!
    let timeline = Timeline::new_sequence("time");
    let time = TimePoint::from([(timeline, 0)]);

    // Log entities with boxes and coordinate frames
    for (name, color) in [
        ("red", components::Color::from_rgb(255, 0, 0)),
        ("green", components::Color::from_rgb(0, 255, 0)),
        ("blue", components::Color::from_rgb(0, 0, 255)),
    ] {
        test_context.log_entity(name, |builder| {
            builder
                .with_archetype_auto_row(
                    time.clone(),
                    &archetypes::Boxes3D::from_half_sizes([(1.0, 1.0, 1.0)])
                        .with_colors([color])
                        .with_fill_mode(components::FillMode::Solid),
                )
                .with_archetype_auto_row(
                    time.clone(),
                    &archetypes::CoordinateFrame::new(format!("{name}_frame")),
                )
        });
    }

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

    test_context.send_time_commands(
        test_context.active_store_id(),
        [
            TimeControlCommand::SetActiveTimeline(*timeline.name()),
            TimeControlCommand::SetTime(0.into()),
        ],
    );

    let view_id = setup_blueprint(&mut test_context);
    let mut test_harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(300.0, 300.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });
    test_harness.run();
    test_harness.snapshot("transform_many_child_parent_relations_on_single_time_and_entity");
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_blueprint =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());

        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

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

        view_id
    })
}
