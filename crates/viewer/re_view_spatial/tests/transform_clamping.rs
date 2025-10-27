use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_transform_clamping() {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    {
        test_context.log_entity("boxes/clamped_colors", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Boxes3D::from_half_sizes([(1.0, 1.0, 1.0), (2.0, 2.0, 2.0)])
                    .with_colors([0xFF0000FF]),
            )
        });

        test_context.log_entity("boxes/ignored_colors", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                    [(5.0, 0.0, 0.0)],
                    [(1.0, 1.0, 1.0)],
                )
                .with_colors([0x00FF00FF, 0xFF00FFFF]),
            )
        });

        test_context.log_entity("boxes/more_transforms_than_sizes", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                        [(0.0, 5.0, 0.0)], // translation <- `InstancePoseTranslation3D`
                        [(1.0, 1.0, 1.0)], // scale <- `HalfSize3D`
                    )
                    .with_colors([0x0000FFFF]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    // Note that the scale is applied _after_ the translation.
                    // This means that the scales "scales the translation".
                    // Prior to 0.24, the translation and the scale were on "the same transform",
                    // therefore we'd apply scale first.
                    &re_types::archetypes::InstancePoses3D::new()
                        .with_scales([(1.0, 1.0, 1.0), (2.0, 2.0, 2.0)]),
                )
        });

        test_context.log_entity("boxes/no_primaries", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::Boxes3D::update_fields()
                        .with_centers([(5.0, 0.0, 0.0)])
                        .with_colors([0xFF00FFFF]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::InstancePoses3D::new()
                        .with_scales([(1.0, 1.0, 1.0), (2.0, 2.0, 2.0)]),
                )
        });
    }

    {
        test_context.log_entity("spheres/clamped_colors", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Ellipsoids3D::from_half_sizes([
                    (1.0, 1.0, 1.0),
                    (2.0, 2.0, 2.0),
                ])
                .with_colors([0xFF0000FF]),
            )
        });

        test_context.log_entity("spheres/ignored_colors", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::default(),
                &re_types::archetypes::Ellipsoids3D::from_centers_and_half_sizes(
                    [(5.0, 0.0, 0.0)],
                    [(1.0, 1.0, 1.0)],
                )
                .with_colors([0x00FF00FF, 0xFF00FFFF]),
            )
        });

        test_context.log_entity("spheres/more_transforms_than_sizes", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::Ellipsoids3D::from_centers_and_half_sizes(
                        [(0.0, 5.0, 0.0)],
                        [(1.0, 1.0, 1.0)],
                    )
                    .with_colors([0x0000FFFF]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::InstancePoses3D::new()
                        .with_scales([(1.0, 1.0, 1.0), (2.0, 2.0, 2.0)]),
                )
        });

        test_context.log_entity("spheres/no_primaries", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::Ellipsoids3D::update_fields()
                        .with_centers([(5.0, 0.0, 0.0)])
                        .with_colors([0xFF00FFFF]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::InstancePoses3D::new()
                        .with_scales([(1.0, 1.0, 1.0), (2.0, 2.0, 2.0)]),
                )
        });

        test_context.log_entity("points/more_transforms_than_positions", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::Points3D::new([(0.0, 5.0, 0.0), (0.1, 5.1, 0.1)])
                        .with_colors([0x0000FFFF, 0xFF0000FF])
                        .with_radii([-15.]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::default(),
                    &re_types::archetypes::InstancePoses3D::new().with_translations([
                        (1.0, 1.0, 1.0),
                        (2.0, 2.0, 2.0),
                        (3.0, 3.0, 3.0),
                    ]),
                )
        });
    }

    let view_ids = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &test_context,
        view_ids,
        "transform_clamping",
        egui::vec2(300.0, 300.0),
    );
}

#[expect(clippy::unwrap_used)]
fn setup_blueprint(test_context: &mut TestContext) -> (ViewId, ViewId, ViewId) {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint_boxes = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: "/boxes".into(),
                query_filter: "+ $origin/**".parse().unwrap(),
            },
        );

        let view_blueprint_spheres = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: "/spheres".into(),
                query_filter: "+ $origin/**".parse().unwrap(),
            },
        );

        let view_blueprint_points = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: "/points".into(),
                query_filter: "+ $origin/**".parse().unwrap(),
            },
        );

        let view_id_boxes = view_blueprint_boxes.id;
        let view_id_spheres = view_blueprint_spheres.id;
        let view_id_points = view_blueprint_points.id;

        blueprint.add_views(
            [
                view_blueprint_boxes,
                view_blueprint_spheres,
                view_blueprint_points,
            ]
            .into_iter(),
            None,
            None,
        );

        (view_id_boxes, view_id_spheres, view_id_points)
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &TestContext,
    (view_id_boxes, view_id_spheres, view_id_points): (ViewId, ViewId, ViewId),
    name: &str,
    size: egui::Vec2,
) {
    for (target, view_id) in [("boxes", view_id_boxes), ("spheres", view_id_spheres)] {
        let mut harness = test_context
            .setup_kittest_for_rendering_3d(size)
            .build_ui(|ui| {
                test_context.run_with_single_view(ui, view_id);
            });

        {
            // This test checks the clamping behavior of components & instance poses on boxes & spheres.
            //
            // One view shows spheres, the other boxes.
            //
            // For both you should see:
            // * 2x red (one bigger than the other)
            // * 1x green
            // * 2x blue (one bigger than the other)
            // * NO other boxes/spheres, in particular no magenta ones!

            let name = format!("{name}_{target}");
            let raw_input = harness.input_mut();
            raw_input
                .events
                .push(egui::Event::PointerMoved((100.0, 100.0).into()));
            raw_input.events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::Vec2::UP * 2.0,
                modifiers: egui::Modifiers::default(),
            });
            harness.run_steps(10);

            harness.snapshot(&name);
        }
    }

    // Points don't have scale, so we test them separately.
    {
        let mut harness = test_context
            .setup_kittest_for_rendering_3d(size)
            .build_ui(|ui| {
                test_context.run_with_single_view(ui, view_id_points);
            });

        // For both you should see:
        // * 3x red
        // * 3x blue
        // * these points should be in three distinct clusters.

        let name = format!("{name}_points");
        let raw_input = harness.input_mut();
        raw_input
            .events
            .push(egui::Event::PointerMoved((100.0, 100.0).into()));
        raw_input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::Vec2::UP * 2.0,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(10);

        harness.snapshot(&name);
    }
}
