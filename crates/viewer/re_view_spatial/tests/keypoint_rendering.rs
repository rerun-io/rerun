use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_sdk_types::archetypes::{AnnotationContext, Points2D, Points3D};
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_sdk_types::components::{Position3D, Radius};
use re_sdk_types::datatypes::{ClassDescription, KeypointPair, Rgba32};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::ViewClass as _;
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

#[test]
fn test_keypoint_annotations_and_connections_2d() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView2D>();

    log_annotation_context(&mut test_context);

    test_context.log_entity("points", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Points2D::new([(0.0, 0.0), (100.0, 0.0), (50.0, 50.0), (50.0, 100.0)])
                .with_class_ids([0])
                .with_keypoint_ids([0, 1, 2, 3])
                .with_radii([Radius::new_ui_points(8.0)])
                .with_show_labels(true),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            re_view_spatial::SpatialView2D::identifier(),
        ))
    });

    test_context
        .run_view_ui_and_save_renderer_snapshot(
            view_id,
            "keypoint_annotations_and_connections",
            egui::vec2(300.0, 300.0),
            None,
        )
        .unwrap();
}

#[test]
fn test_keypoint_annotations_and_connections_3d() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    log_annotation_context(&mut test_context);

    test_context.log_entity("points", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &Points3D::new([
                (-1.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (0.0, 1.0, 1.0),
                (0.0, -1.0, -1.0),
            ])
            .with_class_ids([0])
            .with_keypoint_ids([0, 1, 2, 3])
            .with_radii([Radius::new_ui_points(8.0)])
            .with_show_labels(true),
        )
    });

    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view =
            ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
        let view_id = view.id;
        blueprint.add_views(std::iter::once(view), None, None);

        let eye_property = ViewProperty::from_archetype_for_view::<EyeControls3D>(ctx, view_id);
        eye_property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_position(),
            &Position3D::new(3.0, -5.0, 3.0),
        );
        eye_property.save_blueprint_component(
            ctx,
            &EyeControls3D::descriptor_look_target(),
            &Position3D::new(0.0, 0.0, 0.0),
        );

        view_id
    });

    test_context
        .run_view_ui_and_save_renderer_snapshot(
            view_id,
            "keypoint_annotations_and_connections_3d",
            egui::vec2(300.0, 300.0),
            None,
        )
        .unwrap();
}

fn log_annotation_context(test_context: &mut TestContext) {
    test_context.log_entity("/", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &AnnotationContext::new([ClassDescription {
                info: (0, "pose", Rgba32::WHITE).into(),
                keypoint_annotations: vec![
                    (0, "left", Rgba32::from_rgb(255, 0, 0)).into(),
                    (1, "right", Rgba32::from_rgb(0, 255, 0)).into(),
                    (2, "center", Rgba32::from_rgb(0, 128, 255)).into(),
                    (3, "bottom", Rgba32::from_rgb(255, 255, 0)).into(),
                ],
                keypoint_connections: KeypointPair::vec_from([(0, 2), (1, 2), (2, 3)]),
            }]),
        )
    });
}
