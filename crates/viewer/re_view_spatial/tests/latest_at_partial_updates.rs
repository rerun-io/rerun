use re_chunk_store::RowId;
use re_log_types::{EntityPath, build_frame_nr};
use re_sdk_types::{Archetype as _, archetypes};
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::SnapshotOptions;
use re_test_viewport::TestContextExt as _;
use re_view_spatial::SpatialView2D;
use re_viewer_context::{BlueprintContext as _, TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

#[test]
fn test_latest_at_partial_update() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView2D>();

    let entity_path = EntityPath::from("points");

    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [build_frame_nr(42)],
            &archetypes::Points2D::new([(0., 0.), (1., 1.)]),
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [build_frame_nr(43)],
            &archetypes::Points2D::update_fields().with_radii([-20.]),
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [build_frame_nr(44)],
            &archetypes::Points2D::update_fields().with_colors([(0, 0, 255)]),
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [build_frame_nr(45)],
            &archetypes::Points2D::new([(0., 1.), (1., 0.)]),
        )
    });

    let timepoint = [build_frame_nr(46)];
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint,
            &[archetypes::Points2D::new([(0., 0.), (1., 1.)])],
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint,
            &[archetypes::Points2D::update_fields().with_radii([-30.])],
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint,
            &[archetypes::Points2D::update_fields().with_colors([(0, 255, 0)])],
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint,
            &archetypes::Points2D::update_fields().with_positions([(1., 1.), (1., 0.)]),
        )
    });

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &test_context,
        view_id,
        "latest_at_partial_updates",
        egui::vec2(200.0, 200.0),
    );
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(SpatialView2D::identifier());

        let property_path = re_viewport_blueprint::entity_path_for_view_property(
            view.id,
            ctx.store_context.blueprint.tree(),
            re_sdk_types::blueprint::archetypes::VisualBounds2D::name(),
        );
        ctx.save_blueprint_archetype(
            property_path.clone(),
            &re_sdk_types::blueprint::archetypes::VisualBounds2D::new(
                re_sdk_types::datatypes::Range2D {
                    x_range: [-0.5, 1.5].into(),
                    y_range: [-0.5, 1.5].into(),
                },
            ),
        );

        ctx.save_blueprint_archetype(
            view.defaults_path.clone(),
            &archetypes::Points2D::update_fields()
                .with_colors([(255, 255, 0)])
                .with_radii([-10.]),
        );

        blueprint.add_view_at_root(view)
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering_3d(size)
        .build_ui(|ui| {
            test_context.run_with_single_view(ui, view_id);
        });

    {
        let options = SnapshotOptions::new().output_path(format!("tests/snapshots/{name}"));

        let mut success = true;
        for frame_nr in 42..=46 {
            {
                let (timeline, time) = build_frame_nr(frame_nr);
                test_context.send_time_commands(
                    test_context.active_store_id(),
                    [
                        TimeControlCommand::SetActiveTimeline(*timeline.name()),
                        TimeControlCommand::SetTime(time.into()),
                    ],
                );
            }

            harness.run_steps(8);

            if harness
                .try_snapshot_options(format!("frame_{frame_nr}"), &options)
                .is_err()
            {
                success = false;
            }
        }

        assert!(success, "at least one frame failed");
    }
}
