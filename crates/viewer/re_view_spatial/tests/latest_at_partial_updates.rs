#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use re_chunk_store::{Chunk, RowId};
use re_log_types::{EntityPath, Timeline};
use re_view_spatial::SpatialView3D;
use re_viewer_context::test_context::TestContext;
use re_viewer_context::{RecommendedView, ViewClass, ViewId};
use re_viewport_blueprint::test_context_ext::TestContextExt;
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_latest_at_partial_updates() {
    let mut test_context = get_test_context();

    let path: EntityPath = "points".into();
    let timeline_step = Timeline::new_sequence("step");

    // Checks that inter- and intra-timestamp partial updates are properly handled by latest-at queries,
    // end-to-end: all the way to the views and the renderer.

    // 0
    test_context.log_entity(path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline_step, 0)],
            &re_types::archetypes::Points3D::new([(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)]),
        )
    });

    // 1
    test_context.log_entity(path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline_step, 1)],
            &re_types::archetypes::Points3D::update_fields().with_radii([-4.0]),
        )
    });

    // 2
    test_context.log_entity(path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline_step, 2)],
            &re_types::archetypes::Points3D::update_fields().with_colors([0x0000FFFF]),
        )
    });

    // 3
    test_context.log_entity(path.clone(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline_step, 3)],
            &re_types::archetypes::Points3D::new([(0.0, 0.0, 1.0), (1.0, 1.0, 0.0)]),
        )
    });

    // 4
    test_context.log_entity(path.clone(), |builder| {
        builder
            .with_archetype(
                RowId::new(),
                [(timeline_step, 4)],
                &re_types::archetypes::Points3D::new([(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)]),
            )
            .with_archetype(
                RowId::new(),
                [(timeline_step, 4)],
                &re_types::archetypes::Points3D::update_fields().with_radii([-6.0]),
            )
            .with_archetype(
                RowId::new(),
                [(timeline_step, 4)],
                &re_types::archetypes::Points3D::update_fields().with_colors([0x00FF00FF]),
            )
            .with_archetype(
                RowId::new(),
                [(timeline_step, 4)],
                &re_types::archetypes::Points3D::new([(0.0, 0.0, 1.0), (1.0, 1.0, 0.0)]),
            )
    });

    let view_id = setup_blueprint(&mut test_context);

    run_view_ui_and_save_snapshot(
        &mut test_context,
        timeline_step,
        view_id,
        "latest_at_partial_updates",
        egui::vec2(300.0, 140.0),
    );
}

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_spatial::SpatialView3D>();

    test_context
}

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    let view_blueprint = ViewBlueprint::new(
        re_view_spatial::SpatialView3D::identifier(),
        RecommendedView::root(),
    );

    // Set view defaults.
    {
        _ = test_context
            .blueprint_store
            .add_chunk(&Arc::new(
                Chunk::builder(ViewBlueprint::defaults_path(view_blueprint.id))
                    .with_archetype(
                        RowId::new(),
                        [(Timeline::new_sequence("blueprint"), 0)],
                        &re_types::archetypes::Points3D::update_fields()
                            .with_colors([0xFFFF00FF])
                            .with_radii([-2.0]),
                    )
                    .build()
                    .unwrap(),
            ))
            .unwrap();
    }

    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        view_id
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    timeline: Timeline,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    test_context.set_active_timeline(timeline);

    let rec_cfg = test_context.recording_config.clone();

    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            re_ui::apply_style_and_install_loaders(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                test_context.run(ctx, |ctx| {
                    let view_class = ctx
                        .view_class_registry
                        .get_class_or_log_error(SpatialView3D::identifier());

                    let view_blueprint = ViewBlueprint::try_from_db(
                        view_id,
                        ctx.store_context.blueprint,
                        ctx.blueprint_query,
                    )
                    .expect("we just created that view");

                    let mut view_states = test_context.view_states.lock();

                    let view_state = view_states.get_mut_or_create(view_id, view_class);
                    let (view_query, system_execution_output) =
                        re_viewport::execute_systems_for_view(ctx, &view_blueprint, view_state);
                    view_class
                        .ui(ctx, ui, view_state, &view_query, system_execution_output)
                        .expect("failed to run view ui");
                });

                test_context.handle_system_commands();
            });
        });

    {
        let raw_input = harness.input_mut();
        raw_input
            .events
            .push(egui::Event::PointerMoved((100.0, 100.0).into()));
        raw_input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::Vec2::UP * 3.0,
            modifiers: egui::Modifiers::default(),
        });
        harness.run_steps(8);

        for time in 0..=4 {
            let name = format!("{name}_{}_{time}", timeline.name());

            rec_cfg
                .time_ctrl
                .write()
                .set_time_for_timeline(timeline, time);

            harness.run_steps(8);

            let broken_percent_threshold = 0.0036;
            let num_pixels = (size.x * size.y).ceil() as u64;

            use re_viewer_context::test_context::HarnessExt as _;
            harness.snapshot_with_broken_pixels_threshold(
                &name,
                num_pixels,
                broken_percent_threshold,
            );
        }
    }
}
