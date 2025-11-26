//! Ensures that 2D/3D visualizer report errors on incompatible topology.

use re_log_types::TimePoint;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::{ViewClassIdentifier, archetypes};
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

#[derive(Debug, Clone, Copy)]
struct ExpectedError {
    visualizer: &'static str,
    entity_path: &'static str,
    message_substring: &'static str,
}

struct TestScenario {
    description: &'static str,
    space_origin: &'static str,
    view_class: ViewClassIdentifier,
    expected_errors: Vec<ExpectedError>,
}

fn setup_scene(test_context: &mut TestContext) {
    // We're using explicit transform frames here because it can trigger more different errors,
    // but most things work with implicit transform frames just as well.
    //
    // Transform frame forest:
    // world
    //  ├─ points3d
    //  └─ tf#/pinhole_workaround  # TODO(RR-2680): use explicit frames instead.
    //      └─ tf#/pinhole_workaround/pinhole_entity
    //          └─ points2d
    //          └─ misplaced_boxes3d
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
                    .with_child_frame("points2d")
                    .with_parent_frame("tf#/pinhole_workaround/pinhole_entity"), // TODO(RR-2680): use explicit frames instead.
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                &archetypes::Transform3D::new()
                    .with_child_frame("misplaced_boxes3d")
                    .with_parent_frame("tf#/pinhole_workaround/pinhole_entity"), // TODO(RR-2680): use explicit frames instead.
            )
            .with_archetype_auto_row(
                TimePoint::STATIC,
                // TODO(RR-2680): use explicit frames instead, removing this connection.
                &archetypes::Transform3D::new()
                    .with_child_frame("tf#/pinhole_workaround")
                    .with_parent_frame("world"),
            )
    });

    test_context.log_entity("pinhole_workaround/pinhole_entity", |builder| {
        builder.with_archetype_auto_row(
            TimePoint::STATIC,
            &archetypes::Pinhole::from_focal_length_and_resolution([1.0, 1.0], [100.0, 100.0]),
            // TODO(RR-2680): set child/parent frames.
        )
    });

    // TODO(RR-2997): If we could set the view's origin directly to a frame, we woudl set it to `world`. As there's nothing to be visualized on `world_entity` this would make this log call redundant.
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
}

#[test]
fn test_topology_errors() {
    let mut test_context = TestContext::new();
    test_context.register_view_class::<re_view_spatial::SpatialView3D>();
    test_context.register_view_class::<re_view_spatial::SpatialView2D>();

    setup_scene(&mut test_context);

    // Most scenarios run into the unreachable & misplaced 3D entities not being visualizable!
    let disconnected_entity_error = ExpectedError {
        visualizer: "Ellipsoids3D",
        entity_path: "disconnected_entity",
        message_substring: "No transform path to the view's origin frame.",
    };
    let misplaced_boxes = ExpectedError {
        visualizer: "Boxes3D",
        entity_path: "misplaced_boxes3d_entity",
        message_substring: "Can't visualize 3D content that is under a pinhole projection.",
    };

    let scenarios = [
        TestScenario {
            description: "3D view with 3D content under world including a Pinhole with 2D content",
            space_origin: "world_entity",
            view_class: re_view_spatial::SpatialView3D::identifier(),
            expected_errors: vec![disconnected_entity_error, misplaced_boxes],
        },
        TestScenario {
            description: "2D view with 2D content under world and 3D projected content",
            space_origin: "pinhole_workaround/pinhole_entity",
            view_class: re_view_spatial::SpatialView2D::identifier(),
            expected_errors: vec![disconnected_entity_error, misplaced_boxes],
        },
        TestScenario {
            description: "3D view at a Pinhole",
            space_origin: "pinhole_workaround/pinhole_entity",
            view_class: re_view_spatial::SpatialView3D::identifier(),
            expected_errors: vec![
                disconnected_entity_error,
                ExpectedError {
                    visualizer: "Boxes3D",
                    entity_path: "misplaced_boxes3d_entity",
                    message_substring: "The origin of the 3D view is under pinhole projection which is not supported by most 3D visualizations.",
                },
                ExpectedError {
                    visualizer: "Points2D",
                    entity_path: "points2d_entity",
                    message_substring: "The origin of the 3D view is under pinhole projection which is not supported by most 3D visualizations.",
                },
                ExpectedError {
                    visualizer: "Points3D",
                    entity_path: "points3d_entity",
                    message_substring: "The origin of the 3D view is under pinhole projection which is not supported by most 3D visualizations.",
                },
            ],
        },
        TestScenario {
            description: "pinhole inside 2D view",
            space_origin: "world_entity",
            view_class: re_view_spatial::SpatialView2D::identifier(),
            expected_errors: vec![
                disconnected_entity_error,
                misplaced_boxes,
                ExpectedError {
                    visualizer: "Points2D",
                    entity_path: "points2d_entity",
                    message_substring: "Can't visualize 2D content with a pinhole ancestor that's embedded within the 2D view.",
                },
                ExpectedError {
                    visualizer: "Points3D",
                    entity_path: "points3d_entity",
                    message_substring: "3D visualizers require a pinhole at the origin of the 2D view.",
                },
            ],
        },
    ];

    // Want to run all scenarios even if some fail.
    let mut test_failed = false;

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

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([100.0, 100.0])
            .build_ui(|ui| {
                test_context.run_with_single_view(ui, view_id);
            });
        harness.run();

        let mut visualizer_errors = test_context
            .view_states
            .lock()
            .visualizer_errors(view_id)
            .cloned()
            .unwrap_or_default();

        for expected_error in scenario.expected_errors {
            let ExpectedError {
                visualizer,
                entity_path,
                message_substring,
            } = expected_error;
            let visualizer = visualizer.into();
            let entity_path = entity_path.into();

            let std::collections::hash_map::Entry::Occupied(mut errors_for_visualizer) =
                visualizer_errors.0.entry(visualizer)
            else {
                println!(
                    "In scenario {:?}, expected visualizer errors not found: {expected_error:?}",
                    scenario.description
                );
                test_failed = true;
                continue;
            };

            let actual_error = errors_for_visualizer.get().error_string_for(&entity_path);

            match actual_error {
                Some(actual_error) if actual_error.contains(message_substring) => {
                    // All good. Remove error from list so we can later check for unexpected errors.
                    match errors_for_visualizer.get_mut() {
                        re_viewer_context::VisualizerExecutionErrorState::Overall(_) => {
                            // Not handled right now.
                            println!(
                                "In scenario {:?}, for visualizer={visualizer:?} entity_path={entity_path:?}: but got overall error instead of expected per-entity error {message_substring:?}.",
                                scenario.description,
                            );
                            test_failed = true;
                        }
                        re_viewer_context::VisualizerExecutionErrorState::PerEntity(per_entity) => {
                            per_entity.remove(&entity_path);
                            if per_entity.is_empty() {
                                errors_for_visualizer.remove();
                            }
                        }
                    }
                }
                Some(actual_error) => {
                    println!(
                        "In scenario {:?}, for visualizer={visualizer:?} entity_path={entity_path:?}: expected to contain {message_substring:?}, got {actual_error:?}",
                        scenario.description,
                    );
                    test_failed = true;
                }
                None => {
                    println!(
                        "In scenario {:?}, for visualizer={visualizer:?} entity_path={entity_path:?}: expected error {message_substring:?}",
                        scenario.description,
                    );
                    test_failed = true;
                }
            }
        }

        // Check for unexpected errors.
        for (visualizer, errors) in visualizer_errors.0 {
            println!(
                "In scenario {:?}, unexpected errors for visualizer {visualizer:?}: {errors:?}",
                scenario.description
            );
            test_failed = true;
        }
    }

    assert!(
        !test_failed,
        "One or more test scenarios failed. See previous log messages."
    );
}
