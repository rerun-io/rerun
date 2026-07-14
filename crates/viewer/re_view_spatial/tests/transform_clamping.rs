use re_chunk_store::RowId;
use re_log_types::{EntityPath, TimePoint};
use re_sdk_types::blueprint::archetypes::EyeControls3D;
use re_sdk_types::components::Position3D;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotResults;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{BlueprintContext as _, RecommendedView, ViewClass as _, ViewId};
use re_viewport_blueprint::{ViewBlueprint, ViewProperty};

/// Whether everything is affected by a base-transform and how it is expressed.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum BaseTransform {
    None,
    EntityHierarchy,
    FrameHierarchy,
}

fn test_transform_clamping(base_transform: BaseTransform) {
    let mut test_context = TestContext::new_with_view_class::<re_view_spatial::SpatialView3D>();

    {
        test_context.log_entity("base/boxes/clamped_colors", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &re_sdk_types::archetypes::Boxes3D::from_half_sizes([
                    (1.0, 1.0, 1.0),
                    (2.0, 2.0, 2.0),
                ])
                .with_colors([0xFF0000FF]),
            )
        });

        test_context.log_entity("base/boxes/ignored_colors", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &re_sdk_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                    [(5.0, 0.0, 0.0)],
                    [(1.0, 1.0, 1.0)],
                )
                .with_colors([0x00FF00FF, 0xFF00FFFF]),
            )
        });

        test_context.log_entity("base/boxes/more_transforms_than_sizes", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                        [(0.0, 5.0, 0.0)], // translation <- `InstancePoses3D`-like Translation3D`
                        [(1.0, 1.0, 1.0)], // scale <- `HalfSize3D`
                    )
                    .with_colors([0x0000FFFF]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    // Note that the scale is applied _after_ the translation.
                    // This means that the scales "scales the translation".
                    // Prior to 0.24, the translation and the scale were on "the same transform",
                    // therefore we'd apply scale first.
                    &re_sdk_types::archetypes::InstancePoses3D::new()
                        .with_scales([(1.0, 1.0, 1.0), (2.0, 2.0, 2.0)]),
                )
        });

        test_context.log_entity("base/boxes/no_primaries", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::Boxes3D::update_fields()
                        .with_centers([(5.0, 0.0, 0.0)])
                        .with_colors([0xFF00FFFF]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::InstancePoses3D::new()
                        .with_scales([(1.0, 1.0, 1.0), (2.0, 2.0, 2.0)]),
                )
        });
    }

    {
        test_context.log_entity("base/spheres/clamped_colors", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &re_sdk_types::archetypes::Ellipsoids3D::from_half_sizes([
                    (1.0, 1.0, 1.0),
                    (2.0, 2.0, 2.0),
                ])
                .with_colors([0xFF0000FF]),
            )
        });

        test_context.log_entity("base/spheres/ignored_colors", |builder| {
            builder.with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &re_sdk_types::archetypes::Ellipsoids3D::from_centers_and_half_sizes(
                    [(5.0, 0.0, 0.0)],
                    [(1.0, 1.0, 1.0)],
                )
                .with_colors([0x00FF00FF, 0xFF00FFFF]),
            )
        });

        test_context.log_entity("base/spheres/more_transforms_than_sizes", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::Ellipsoids3D::from_centers_and_half_sizes(
                        [(0.0, 5.0, 0.0)],
                        [(1.0, 1.0, 1.0)],
                    )
                    .with_colors([0x0000FFFF]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::InstancePoses3D::new()
                        .with_scales([(1.0, 1.0, 1.0), (2.0, 2.0, 2.0)]),
                )
        });

        test_context.log_entity("base/spheres/no_primaries", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::Ellipsoids3D::update_fields()
                        .with_centers([(5.0, 0.0, 0.0)])
                        .with_colors([0xFF00FFFF]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::InstancePoses3D::new()
                        .with_scales([(1.0, 1.0, 1.0), (2.0, 2.0, 2.0)]),
                )
        });

        test_context.log_entity("base/points/more_transforms_than_positions", |builder| {
            builder
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::Points3D::new([(0.0, -2.0, 0.0), (0.0, 1.0, 0.0)])
                        .with_colors([0x0000FFFF, 0xFF0000FF])
                        .with_radii([-15.]),
                )
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::InstancePoses3D::new().with_translations([
                        (1.0, 1.0, 1.0),
                        (2.0, 2.0, 2.0),
                        (3.0, 3.0, 3.0),
                    ]),
                )
        });

        test_context.log_entity(
            "base/transform_axes/more_transforms_than_axis_lengths",
            |builder| {
                builder
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        &re_sdk_types::archetypes::TransformAxes3D::new(1.0),
                    )
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        &re_sdk_types::archetypes::InstancePoses3D::new()
                            .with_translations([
                                (-1.0, -1.0, 0.0),
                                (1.0, -1.0, 0.0),
                                (-1.0, 1.0, 0.0),
                                (1.0, 1.0, 0.0),
                            ])
                            .with_scales([(1.75, 1.75, 1.75), (0.5, 0.5, 0.5)]),
                    )
            },
        );
    }

    match base_transform {
        BaseTransform::None => {
            // Done.
        }

        BaseTransform::EntityHierarchy => {
            test_context.log_entity("base", |builder| {
                builder.with_archetype_auto_row(
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::Transform3D::from_rotation(
                        re_sdk_types::components::RotationAxisAngle::new(
                            glam::Vec3::Z,
                            0.5 * std::f32::consts::PI,
                        ),
                    ),
                )
            });
        }

        BaseTransform::FrameHierarchy => {
            // Put everything under a frame that has an identity relationship with a frame called "base".
            // Note that this means we end up with several `InstancePoses3D` which are all relative to "base".
            // Since poses are always relative to their entity's frame, this should work out fine and give us the same result as `BaseTransform::EntityHierarchy`!
            let entity_paths = test_context
                .store_hub
                .lock()
                .entity_db(&test_context.recording_store_id)
                .expect("expected an active recording")
                .sorted_entity_paths()
                .cloned()
                .collect::<Vec<_>>();
            for path in entity_paths {
                test_context.log_entity(path, |builder| {
                    builder.with_archetype_auto_row(
                        TimePoint::STATIC,
                        &re_sdk_types::archetypes::CoordinateFrame::new("base"),
                    )
                });
            }

            // Create a relationship between `tf#/` and `base`.
            test_context.log_entity("transforms", |builder| {
                builder.with_archetype_auto_row(
                    TimePoint::STATIC,
                    &re_sdk_types::archetypes::Transform3D::from_rotation(
                        re_sdk_types::components::RotationAxisAngle::new(
                            glam::Vec3::Z,
                            0.5 * std::f32::consts::PI,
                        ),
                    )
                    .with_parent_frame("tf#/")
                    .with_child_frame("base"),
                )
            });
        }
    }

    let view_ids = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &test_context,
        view_ids,
        &format!("transform_clamping_with_base_transform_{base_transform:?}"),
        egui::vec2(300.0, 300.0),
    );
}

#[expect(clippy::unwrap_used)]
fn setup_blueprint(test_context: &mut TestContext) -> (ViewId, ViewId, ViewId, ViewId) {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_blueprint_boxes = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: EntityPath::root(),
                query_filter: "+ /base/boxes/**".parse().unwrap(),
            },
        );

        let view_blueprint_spheres = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: EntityPath::root(),
                query_filter: "+ /base/spheres/**".parse().unwrap(),
            },
        );

        let view_blueprint_points = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: EntityPath::root(),
                query_filter: "+ /base/points/**".parse().unwrap(),
            },
        );

        let view_blueprint_transform_axes = ViewBlueprint::new(
            re_view_spatial::SpatialView3D::identifier(),
            RecommendedView {
                origin: EntityPath::root(),
                query_filter: "+ /base/transform_axes/**".parse().unwrap(),
            },
        );

        let view_id_boxes = view_blueprint_boxes.id;
        let view_id_spheres = view_blueprint_spheres.id;
        let view_id_points = view_blueprint_points.id;
        let view_id_transform_axes = view_blueprint_transform_axes.id;

        blueprint.add_views(
            [
                view_blueprint_boxes,
                view_blueprint_spheres,
                view_blueprint_points,
                view_blueprint_transform_axes,
            ]
            .into_iter(),
            None,
            None,
        );

        (
            view_id_boxes,
            view_id_spheres,
            view_id_points,
            view_id_transform_axes,
        )
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &TestContext,
    (view_id_boxes, view_id_spheres, view_id_points, view_id_transform_axes): (
        ViewId,
        ViewId,
        ViewId,
        ViewId,
    ),
    name: &str,
    size: egui::Vec2,
) {
    let mut snapshot_results = SnapshotResults::new();
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
            test_context.with_blueprint_ctx(|ctx, _| {
                let property = ViewProperty::from_archetype::<EyeControls3D>(
                    ctx.current_blueprint(),
                    ctx.blueprint_query(),
                    view_id,
                );
                property.save_blueprint_component(
                    &ctx,
                    &EyeControls3D::descriptor_position(),
                    &Position3D::new(12.0, 5.0, 12.0),
                );
                property.save_blueprint_component(
                    &ctx,
                    &EyeControls3D::descriptor_look_target(),
                    &Position3D::new(0.0, 5.0, 0.0),
                );
            });
            harness.run_steps(10);

            harness.snapshot(&name);
            snapshot_results.extend_harness(&mut harness);
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
        test_context.with_blueprint_ctx(|ctx, _| {
            let property = ViewProperty::from_archetype::<EyeControls3D>(
                ctx.current_blueprint(),
                ctx.blueprint_query(),
                view_id_points,
            );
            property.save_blueprint_component(
                &ctx,
                &EyeControls3D::descriptor_position(),
                &Position3D::new(0.0, 0.0, 12.0),
            );
            property.save_blueprint_component(
                &ctx,
                &EyeControls3D::descriptor_look_target(),
                &Position3D::new(0.0, 0.0, 0.0),
            );
        });
        harness.run_steps(10);

        harness.snapshot(&name);
        snapshot_results.extend_harness(&mut harness);
    }

    // Transform axes with instance poses.
    {
        let mut harness = test_context
            .setup_kittest_for_rendering_3d(size)
            .build_ui(|ui| {
                test_context.run_with_single_view(ui, view_id_transform_axes);
            });

        // You should see:
        // * 4 sets of coordinate axes at distinct positions (translated by instance poses)
        // * 1 should be larger then the others

        let name = format!("{name}_transform_axes");
        test_context.with_blueprint_ctx(|ctx, _| {
            let property = ViewProperty::from_archetype::<EyeControls3D>(
                ctx.current_blueprint(),
                ctx.blueprint_query(),
                view_id_transform_axes,
            );
            property.save_blueprint_component(
                &ctx,
                &EyeControls3D::descriptor_position(),
                &Position3D::new(6.0, 6.0, 6.0),
            );
            property.save_blueprint_component(
                &ctx,
                &EyeControls3D::descriptor_look_target(),
                &Position3D::new(2.0, 2.0, 2.0),
            );
        });
        harness.run_steps(10);

        harness.snapshot(&name);
        snapshot_results.extend_harness(&mut harness);
    }
}

#[test]
fn test_transform_clamping_no_base_transform() {
    test_transform_clamping(BaseTransform::None);
}

#[test]
fn test_transform_clamping_entity_hierarchy_base_transform() {
    test_transform_clamping(BaseTransform::EntityHierarchy);
}

#[test]
fn test_transform_clamping_frame_hierarchy_base_transform() {
    test_transform_clamping(BaseTransform::FrameHierarchy);
}
