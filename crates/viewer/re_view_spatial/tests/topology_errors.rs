//! Ensures that 2D/3D visualizer report errors on incompatible topology.

use re_log_types::TimePoint;
use re_test_context::TestContext;
use re_test_viewport::TestContextExt as _;
use re_types::{ViewClassIdentifier, archetypes};
use re_viewer_context::{RecommendedView, ViewClass as _};
use re_viewport_blueprint::ViewBlueprint;

struct TestScenario {
    name: &'static str,
    space_origin: &'static str,
    view_class: ViewClassIdentifier,
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
            space_origin: "pinhole_workaround/pinhole_entity",
            view_class: re_view_spatial::SpatialView2D::identifier(),
        },
        TestScenario {
            name: "3d_view_at_pinhole",
            space_origin: "pinhole_workaround/pinhole_entity",
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

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([100.0, 100.0])
            .build_ui(|ui| {
                test_context.run_with_single_view(ui, view_id);
            });
        harness.run();

        let visualizer_errors = test_context
            .view_states
            .lock()
            .visualizer_errors(view_id)
            .cloned()
            .unwrap_or_default();

        insta::assert_debug_snapshot!(scenario.name, visualizer_errors);
    }
}
