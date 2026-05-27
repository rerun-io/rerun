use re_log_types::TimePoint;
use re_sdk_types::{
    RowId,
    archetypes::{TransformAxes3D, VoxelGridMap},
    blueprint::archetypes::{EyeControls3D, LineGrid3D, SpatialInformation},
    blueprint::components::{Enabled, GridSpacing},
    components::{Colormap, Position3D, RotationAxisAngle},
};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{BlueprintContext as _, Item, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

#[test]
fn test_voxel_grid_map_snapshot_and_instance_selection() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    test_context.log_entity("/", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::STATIC, &TransformAxes3D::new(1.0))
    });

    test_context.log_entity("values", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &VoxelGridMap::new(
                [
                    (0, 0, 0),
                    (1, 0, 0),
                    (1, 1, 0),
                    (4, 0, 0),
                    (4, 0, 1),
                    (5, 0, 1),
                ],
                0.5,
            )
            .with_values([0.0, 0.2, 0.4, 0.6, 0.8, 1.0])
            .with_value_range([0.0, 1.0])
            .with_colormap(Colormap::Turbo),
        )
    });

    test_context.log_entity("posed_colors", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &VoxelGridMap::new([(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 0)], 0.6)
                .with_translation([0.0, 2.2, 0.0])
                .with_rotation_axis_angle(RotationAxisAngle::new(
                    glam::Vec3::Z,
                    std::f32::consts::FRAC_PI_4,
                ))
                .with_colors([
                    0xFF0000FF, // red
                    0x00FF00FF, // green
                    0x0000FFFF, // blue
                    0xFF00FF00, // alpha zero, should be skipped
                ]),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView3D::identifier(),
        ))
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(360.0, 260.0))
        .build_ui(|ui| {
            test_context.edit_selection(|selection_state| {
                selection_state.set_selection(Item::InstancePath(
                    re_entity_db::InstancePath::instance("values", 1),
                ));
            });
            test_context.run_with_single_view(ui, view_id);
        });

    test_context.with_blueprint_ctx(|ctx, _| {
        let grid_property = ViewProperty::from_archetype::<LineGrid3D>(
            ctx.current_blueprint(),
            ctx.blueprint_query(),
            view_id,
        );
        grid_property.save_blueprint_component(
            &ctx,
            &LineGrid3D::descriptor_spacing(),
            &GridSpacing::from(0.5),
        );

        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.current_blueprint(),
            ctx.blueprint_query(),
            view_id,
        );
        eye_property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(3.0, -5.0, 5.0),
        );
        eye_property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(2.0, 0.8, 0.4),
        );

        let spatial_info_property = ViewProperty::from_archetype::<SpatialInformation>(
            ctx.current_blueprint(),
            ctx.blueprint_query(),
            view_id,
        );
        spatial_info_property.save_blueprint_component(
            &ctx,
            &SpatialInformation::descriptor_show_axes(),
            &Enabled::from(true),
        );
    });
    harness.run_steps(10);

    harness.snapshot("voxel_grid_map");
}

#[test]
fn test_voxel_grid_map_transparent_opacity_snapshot() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    test_context.log_entity("opaque_back", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &VoxelGridMap::new([(0, 0, 0)], 1.0)
                .with_translation([0.0, 0.35, 0.0])
                .with_colors([0x00FF00FF]),
        )
    });

    test_context.log_entity("transparent_front", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &VoxelGridMap::new([(0, 0, 0)], 1.0)
                .with_translation([0.0, -0.35, 0.0])
                .with_colors([0xFF0000FF])
                .with_opacity(0.35),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView3D::identifier(),
        ))
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(260.0, 220.0))
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    test_context.with_blueprint_ctx(|ctx, _| {
        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.current_blueprint(),
            ctx.blueprint_query(),
            view_id,
        );
        eye_property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(1.7, -4.0, 2.2),
        );
        eye_property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(0.5, 0.2, 0.5),
        );
    });
    harness.run_steps(10);

    harness.snapshot("voxel_grid_map_transparent_opacity");
}
