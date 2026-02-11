//! Ensures that 2D/3D visualizer report errors on incompatible topology.

use re_chunk_store::external::re_chunk::external::crossbeam::atomic::AtomicCell;
use re_log_types::TimePoint;
use re_sdk_types::{ViewClassIdentifier, archetypes};
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

struct TestScenario {
    name: &'static str,
    space_origin: &'static str,
    view_class: ViewClassIdentifier,
}

fn setup_scene(test_context: &mut TestContext) {
    // We're using named transform frames here because it can trigger more different errors,
    // but most things work with implicit transform frames just as well.
    //
    // Transform frame forest:
    // world
    //  ├─ points3d
    //  ├─ misplaced_image
    //  ├─ misplaced_depth_image
    //  └─ pinhole
    //     ├─ points2d
    //     └─ misplaced_boxes3d
    // disconnected

    test_context.log_entity("transforms", |builder| {
        builder
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Transform3D::new()
                    .with_child_frame("points3d")
                    .with_parent_frame("world"),
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Transform3D::new()
                    .with_child_frame("misplaced_image")
                    .with_parent_frame("world"),
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Transform3D::new()
                    .with_child_frame("misplaced_depth_image")
                    .with_parent_frame("world"),
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Transform3D::new()
                    .with_child_frame("points2d")
                    .with_parent_frame("pinhole"),
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Transform3D::new()
                    .with_child_frame("misplaced_boxes3d")
                    .with_parent_frame("pinhole"),
            )
    });
    test_context.log_entity("pinhole_entity", |builder| {
        builder
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Pinhole::from_focal_length_and_resolution([1.0, 1.0], [100.0, 100.0])
                    .with_child_frame("pinhole")
                    .with_parent_frame("world"),
            )
            // TODO(RR-2997): Space origin can only be an entity, so we rely on pinhole having a coordinate frame that we can pick up.
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::CoordinateFrame::new("pinhole"),
            )
    });

    // TODO(RR-2997): If we could set the view's origin directly to a frame, we would set it to `world`. As there's nothing to be visualized on `world_entity` this would make this log call redundant.
    test_context.log_entity("world_entity", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &archetypes::CoordinateFrame::new("world"),
        )
    });
    test_context.log_entity("points3d_entity", |builder| {
        builder
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Points3D::new([[1.0, 1.0, 1.0]]),
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::CoordinateFrame::new("points3d"),
            )
    });
    test_context.log_entity("points2d_entity", |builder| {
        builder
            .with_archetype_auto_row(TimePoint::STATIC, &archetypes::Points2D::new([[1.0, 1.0]]))
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::CoordinateFrame::new("points2d"),
            )
    });
    test_context.log_entity("disconnected_entity", |builder| {
        builder
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Ellipsoids3D::from_half_sizes([[1.0, 1.0, 1.0]]),
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::CoordinateFrame::new("disconnected"),
            )
    });
    test_context.log_entity("misplaced_boxes3d_entity", |builder| {
        builder
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Boxes3D::from_half_sizes([[1.0, 1.0, 1.0]]),
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::CoordinateFrame::new("misplaced_boxes3d"),
            )
    });
    test_context.log_entity("misplaced_image_entity", |builder| {
        builder
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Image::from_elements(
                    &[255u8, 0, 0],
                    [1, 1],
                    re_sdk_types::datatypes::ColorModel::RGB,
                ),
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::CoordinateFrame::new("misplaced_image"),
            )
    });
    test_context.log_entity("misplaced_depth_image_entity", |builder| {
        builder
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::DepthImage::try_from(re_sdk_types::datatypes::TensorData::new(
                    vec![1u64, 1u64],
                    re_sdk_types::datatypes::TensorBuffer::U16(vec![1u16].into()),
                ))
                .expect("Failed to create depth image from tensor data"),
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::CoordinateFrame::new("misplaced_depth_image"),
            )
    });
}

#[test]
fn test_topology_errors() {
    let mut test_context = TestContext::new();
    test_context.register_view_class::<re_view_spatial::SpatialView3D>();
    test_context.register_view_class::<re_view_spatial::SpatialView2D>();

    setup_scene(&mut test_context);

    let scenarios = [
        TestScenario {
            name: "3d_view_at_root",
            space_origin: "world_entity",
            view_class: re_view_spatial::SpatialView3D::identifier(),
        },
        TestScenario {
            name: "2d_view_at_root",
            space_origin: "world_entity",
            view_class: re_view_spatial::SpatialView2D::identifier(),
        },
        TestScenario {
            name: "2d_view_at_pinhole",
            space_origin: "pinhole_entity",
            view_class: re_view_spatial::SpatialView2D::identifier(),
        },
        TestScenario {
            name: "3d_view_at_pinhole",
            space_origin: "pinhole_entity",
            view_class: re_view_spatial::SpatialView3D::identifier(),
        },
    ];

    for scenario in scenarios {
        let view_id = test_context.setup_viewport_blueprint(|_ctx, blueprint| {
            let view_blueprint = ViewBlueprint::new(
                scenario.view_class,
                RecommendedView {
                    origin: scenario.space_origin.into(),
                    query_filter: re_log_types::EntityPathFilter::all(),
                },
            );
            let view_id = view_blueprint.id;
            blueprint.add_views(std::iter::once(view_blueprint), None, None);
            view_id
        });

        let query_result_tree = AtomicCell::new(Default::default());
        let mut harness = test_context
            .setup_kittest_for_rendering_ui([100.0, 100.0])
            .build_ui(|ui| {
                test_context.run_ui(ui, |ctx, ui| {
                    test_context.ui_for_single_view(ui, ctx, view_id);

                    let query_results = ctx.query_results.get(&view_id).unwrap();
                    query_result_tree.store(query_results.tree.clone());
                });
            });
        harness.run();

        let visualizer_errors = test_context
            .view_states
            .lock()
            .per_visualizer_type_reports(view_id)
            .cloned()
            .unwrap_or_default();

        // Don't show the UUIDs since they're not all that useful in the snapshot.
        let query_result_tree = query_result_tree.take();
        let snapshot_content = visualizer_errors
            .iter()
            .map(|(visualizer_type, error)| {
                let error = match error {
                    re_viewer_context::VisualizerTypeReport::OverallError(err) => {
                        err.summary.clone()
                    }
                    re_viewer_context::VisualizerTypeReport::PerInstructionReport(errors) => errors
                        .iter()
                        .flat_map(|(instr_id, reports)| {
                            let data_result = query_result_tree
                                .lookup_result_by_visualizer_instruction(*instr_id)
                                .unwrap();
                            reports.iter().map(|report| {
                                format!("{:?}: {}", data_result.entity_path, report.summary)
                            })
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                };
                format!("{visualizer_type:?}: {error}")
            })
            .collect::<Vec<_>>()
            .join("\n");

        insta::assert_snapshot!(scenario.name, snapshot_content);
    }
}
