use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::{
    archetypes::{GridMap, TransformAxes3D},
    blueprint::archetypes::{EyeControls3D, LineGrid3D, SpatialInformation},
    blueprint::components::{Enabled, GridSpacing},
    components::{ImageFormat, Position3D, RotationAxisAngle},
    datatypes::{ChannelDatatype, ColorModel},
};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{BlueprintContext as _, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// Verifies that grid map texels are rendered accurately using a tiny source image.
#[test]
fn test_grid_map_texel_accuracy() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    // Create a 5 x 5 pixel checkerboard image.
    let width: u32 = 5;
    let height: u32 = 5;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            pixels.push(if (x + y) % 2 == 0 { 0 } else { 255 });
        }
    }

    // Log as grid map with 1 meter cell size.
    let cell_size = 1.;
    test_context.log_entity("/", |builder| {
        builder.with_archetype(RowId::new(), TimePoint::STATIC, &TransformAxes3D::new(1.0))
    });

    test_context.log_entity("map", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &GridMap::new(
                pixels.clone(),
                ImageFormat::from_color_model([width, height], ColorModel::L, ChannelDatatype::U8),
                cell_size,
            ),
        )
    });

    // Show also another variant with offset and rotation.
    test_context.log_entity("map_rotated", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &GridMap::new(
                pixels.clone(),
                ImageFormat::from_color_model([width, height], ColorModel::L, ChannelDatatype::U8),
                cell_size,
            )
            .with_translation([5.0, 5.0, 0.0])
            .with_rotation_axis_angle(RotationAxisAngle::new(
                glam::Vec3::Z,
                std::f32::consts::FRAC_PI_4,
            )),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView3D::identifier(),
        ))
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(300.0, 200.0))
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    test_context.with_blueprint_ctx(|ctx, _| {
        // Configure world grid to match the cell size.
        let grid_property = ViewProperty::from_archetype::<LineGrid3D>(
            ctx.current_blueprint(),
            ctx.blueprint_query(),
            view_id,
        );
        grid_property.save_blueprint_component(
            &ctx,
            &LineGrid3D::descriptor_spacing(),
            &GridSpacing::from(cell_size),
        );

        // Configure top down eye view.
        let eye_property = ViewProperty::from_archetype::<EyeControls3D>(
            ctx.current_blueprint(),
            ctx.blueprint_query(),
            view_id,
        );
        eye_property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(width as f32, height as f32, 15.0),
        );
        eye_property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(width as f32, height as f32, 0.0),
        );

        // Show spatial origin.
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

    harness.snapshot("grid_map_texel_accuracy");
}
