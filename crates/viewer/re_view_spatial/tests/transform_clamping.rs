use re_chunk_store::RowId;
use re_log_types::TimePoint;
use re_viewer_context::test_context::TestContext;
use re_viewer_context::{RecommendedView, ViewClass as _, ViewId};
use re_viewport::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_transform_clamping() {
    let mut test_context = get_test_context();

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
    }

    let view_ids = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_ids,
        "transform_clamping",
        egui::vec2(300.0, 300.0),
    );
}

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_spatial::SpatialView3D>();

    // Make sure we can draw stuff in the hover tables.
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
    // Also register the legacy UIs.
    re_data_ui::register_component_uis(&mut test_context.component_ui_registry);

    test_context
}

#[allow(clippy::unwrap_used)]
fn setup_blueprint(test_context: &mut TestContext) -> (ViewId, ViewId) {
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

        let view_id_boxes = view_blueprint_boxes.id;
        let view_id_spheres = view_blueprint_spheres.id;

        blueprint.add_views(
            [view_blueprint_boxes, view_blueprint_spheres].into_iter(),
            None,
            None,
        );

        (view_id_boxes, view_id_spheres)
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    (view_id_boxes, view_id_spheres): (ViewId, ViewId),
    name: &str,
    size: egui::Vec2,
) {
    for (target, view_id) in [("boxes", view_id_boxes), ("spheres", view_id_spheres)] {
        let mut harness = test_context
            .setup_kittest_for_rendering()
            .with_size(size)
            .build(|ctx| {
                test_context.run_with_single_view(ctx, view_id);
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
            let broken_pixels_fraction = 0.0045;
            let num_pixels = (size.x * size.y).ceil() as u64;

            use re_viewer_context::test_context::HarnessExt as _;
            harness.snapshot_with_broken_pixels_threshold(
                &name,
                num_pixels,
                broken_pixels_fraction,
            );
        }
    }
}
