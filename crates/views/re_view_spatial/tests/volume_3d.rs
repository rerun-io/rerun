use re_log_types::TimePoint;
use re_sdk_types::{
    RowId,
    archetypes::{Boxes3D, Mesh3D, Volume3D},
    blueprint::archetypes::EyeControls3D,
    components::{Colormap, FillMode, Position3D},
    encodings::{TensorBuffer, TensorData},
};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

#[test]
fn test_volume_3d_snapshot() {
    const SIZE: usize = 32;
    const GRID_SIZE: f32 = 2.0;
    const HALF_SIZE: f32 = GRID_SIZE / 2.0;
    const VOXEL_SIZE: f32 = GRID_SIZE / SIZE as f32;

    let mut voxels = Vec::with_capacity(SIZE * SIZE * SIZE);
    for z in 0..SIZE {
        for y in 0..SIZE {
            for x in 0..SIZE {
                let position = glam::vec3(x as f32, y as f32, z as f32) / (SIZE - 1) as f32 * 2.0
                    - glam::Vec3::ONE;
                let sphere = (1.0 - position.length()).max(0.0);
                let core = (1.0 - (position - glam::vec3(0.5, 0.0, 0.0)).length() * 2.0).max(0.0);
                voxels.push(half::f16::from_f32((sphere * 0.5 + core).min(1.0)));
            }
        }
    }

    let tensor = TensorData::new(
        vec![SIZE as u64, SIZE as u64, SIZE as u64],
        TensorBuffer::F16(voxels.into()),
    );
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();
    test_context.log_entity("volume", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Volume3D::new(tensor)
                .with_voxel_size([VOXEL_SIZE; 3])
                .with_translation([-HALF_SIZE; 3])
                .with_value_range([0.0, 1.0])
                .with_colormap(Colormap::Turbo)
                .with_gamma(1.0)
                .with_optical_density(100.0),
        )
    });
    // Add bounds so it's easier to see in the snippet what space the volume takes.
    test_context.log_entity("volume/bounds", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Boxes3D::from_half_sizes([[HALF_SIZE; 3]])
                .with_fill_mode(FillMode::MajorWireframe)
                .with_colors([0xFFFFFFFF]),
        )
    });
    // Add an occluder to test volume rendering against scene geometry.
    test_context.log_entity("volume/occluder", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            // The plane z = x bisects the grid; the triangle covers its entire cross-section.
            &Mesh3D::new([[-2.0, -2.0, -2.0], [2.0, -2.0, 2.0], [0.0, 4.0, 0.0]])
                .with_triangle_indices([[0, 1, 2]])
                .with_vertex_colors([0x00FFFFFF, 0xFF00FFFF, 0xFFFF00FF]),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView3D::identifier(),
        ))
    });
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(400.0, 400.0))
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    test_context.with_blueprint_ctx(|ctx, _| {
        let eye_property = ViewProperty::from_archetype_for_view::<EyeControls3D>(&ctx, view_id);
        eye_property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(-2.0, 2.0, 2.0),
        );
        eye_property.save_blueprint_component(
            &ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(0.0, 0.0, 0.0),
        );
    });
    harness.run();

    harness.snapshot("volume_3d");
}
