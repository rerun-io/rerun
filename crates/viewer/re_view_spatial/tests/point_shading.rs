use re_log_types::TimePoint;
use re_sdk_types::RowId;
use re_sdk_types::archetypes::Points3D;
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_sdk_types::components::{PointShading, Position3D, Radius};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// Tests the different options for point shading.
#[test]
fn test_point_shading() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    let radius = Radius::new_scene_units(0.2);

    test_context.log_entity("world/gradient", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Points3D::new([[-0.3, 0.0, 0.0]])
                .with_radii([radius])
                .with_colors([0x66CCFFFF])
                .with_point_shading(PointShading::Gradient),
        )
    });

    test_context.log_entity("world/flat", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Points3D::new([[0.3, 0.0, 0.0]])
                .with_radii([radius])
                .with_colors([0x66CCFFFF])
                .with_point_shading(PointShading::Flat),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_blueprint = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView::root(),
        );
        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        let eye_property = ViewProperty::from_archetype_for_view::<EyeControls3D>(ctx, view_id);
        eye_property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(1.0, 1.0, 1.0),
        );
        eye_property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(0.0, 0.0, 0.0),
        );

        view_id
    });

    let mut harness = test_context
        .setup_kittest_for_rendering_3d(egui::vec2(300.0, 300.0))
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    harness.snapshot("point_shading");
}
