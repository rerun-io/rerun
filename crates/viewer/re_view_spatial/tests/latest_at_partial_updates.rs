use re_chunk_store::RowId;
use re_log_types::{EntityPath, build_frame_nr};
use re_types::archetypes;
use re_view_spatial::SpatialView3D;
use re_viewer_context::{
    ViewClass as _, ViewId, external::egui_kittest::SnapshotOptions, test_context::TestContext,
};
use re_viewport::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::ViewBlueprint;

#[test]
fn test_latest_at_partial_update() {
    let mut test_context = TestContext::new_with_view_class::<SpatialView3D>();

    let entity_path = EntityPath::from("points");

    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [build_frame_nr(42)],
            &archetypes::Points3D::new([(0., 0., 0.), (1., 1., 1.)]),
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [build_frame_nr(43)],
            &archetypes::Points3D::update_fields().with_radii([-20.]),
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [build_frame_nr(44)],
            &archetypes::Points3D::update_fields().with_colors([(0, 0, 255)]),
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [build_frame_nr(45)],
            &archetypes::Points3D::new([(0., 0., 1.), (1., 1., 0.)]),
        )
    });

    let timepoint = [build_frame_nr(46)];
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint,
            &[archetypes::Points3D::new([(0., 0., 0.), (1., 1., 1.)])],
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint,
            &[archetypes::Points3D::update_fields().with_radii([-30.])],
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint,
            &[archetypes::Points3D::update_fields().with_colors([(0, 255, 0)])],
        )
    });
    test_context.log_entity(entity_path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            timepoint,
            &archetypes::Points3D::update_fields().with_positions([(0., 0., 1.), (1., 1., 0.)]),
        )
    });

    let view_id = setup_blueprint(&mut test_context);
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "latest_at_partial_updates",
        egui::vec2(600.0, 600.0),
    );
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view = ViewBlueprint::new_with_root_wildcard(SpatialView3D::identifier());

        ctx.save_blueprint_archetype(
            view.defaults_path.clone(),
            &archetypes::Points3D::update_fields()
                .with_colors([(255, 255, 0)])
                .with_radii([-10.]),
        );

        blueprint.add_view_at_root(view)
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let rec_config = test_context.recording_config.clone();

    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            test_context.run_with_single_view(ctx, view_id);
        });
    {
        let raw_input = harness.input_mut();
        raw_input
            .events
            .push(egui::Event::PointerMoved((100.0, 100.0).into()));
        raw_input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::Vec2::UP * 3.1,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(8);

        let broken_pixels_fraction = 0.004;
        let options = SnapshotOptions::new()
            .output_path(format!("tests/snapshots/{name}"))
            .failed_pixel_count_threshold(
                (size.x * size.y * broken_pixels_fraction).round() as usize
            );

        // Frame #42 should look like this:
        // https://static.rerun.io/check_latest_at_partial_updates_frame42/3ed69ef182d8e475a36fd9351669942f5092859f/480w.png

        // Frame #43 should look like this:
        // https://static.rerun.io/check_latest_at_partial_updates_frame43/e86013ac21cc3b6bc17aceecc7cbb9e454128150/480w.png

        // Frame #44 should look like this:
        // https://static.rerun.io/check_latest_at_partial_updates_frame44/df5d4bfe74bcf5fc12ad658f62f35908ceff80bf/480w.png

        // Frame #45 should look like this:
        // https://static.rerun.io/check_latest_at_partial_updates_frame45/8c19fcbe9b7c59ed9e27452a5d2696eee84a4a55/480w.png

        // Frame #46 should look like this:
        // https://static.rerun.io/check_latest_at_partial_updates_frame46/a7f7d8f5b07c1e3fe4ff66e42fd473d2f2edb04b/480w.png

        let mut success = true;
        for frame_nr in 42..=46 {
            let (timeline, _) = build_frame_nr(frame_nr);
            rec_config
                .time_ctrl
                .write()
                .set_time_for_timeline(timeline, frame_nr);

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
