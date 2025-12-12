use re_log_types::TimePoint;
use re_sdk_types::RowId;
use re_sdk_types::archetypes::Mesh3D;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_single_channel_mesh() {
    let texture_format = re_sdk_types::components::ImageFormat::l8([2, 2]);
    let texture_buffer = re_sdk_types::components::ImageBuffer::from(vec![128, 255, 0, 128]);
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    // Log a simple quad mesh with a texture with one channel.
    test_context.log_entity("world/mesh", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::default(),
            &Mesh3D::new([
                [-1.0, 0.0, -1.0],
                [1.0, 0.0, -1.0],
                [-1.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
            ])
            .with_vertex_texcoords([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]])
            .with_triangle_indices([[0, 1, 2], [2, 1, 3]])
            .with_albedo_texture(texture_format, texture_buffer),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView::root(),
        );

        let view_id = view_blueprint.id;

        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        view_id
    });

    let size = egui::vec2(300.0, 300.0);

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    harness.snapshot("mesh3d_grayscale_texture");
}
