#![cfg(feature = "testing")]

use re_chunk_store::RowId;
use re_log_types::{Timeline, TimelineName};
use re_types::archetypes::Scalars;
use re_ui::UiExt as _;
use re_view_dataframe::DataframeView;
use re_viewer_context::test_context::TestContext;
use re_viewer_context::{RecommendedView, ViewClass as _, ViewId};
use re_viewport_blueprint::test_context_ext::TestContextExt as _;
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_null_timeline() {
    let mut test_context = get_test_context();

    let timeline_a = Timeline::new_sequence("timeline_a");
    let timeline_b = Timeline::new_sequence("timeline_b");

    test_context.log_entity("first".into(), |builder| {
        builder.with_archetype(RowId::new(), [(timeline_a, 0)], &Scalars::one(10.0))
    });

    test_context.log_entity("second".into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline_a, 1), (timeline_b, 10)],
            &Scalars::one(12.0),
        )
    });

    let view_id = setup_blueprint(&mut test_context, timeline_a.name());
    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "null_timeline",
        egui::vec2(300.0, 150.0),
    );
}

#[test]
pub fn test_unknown_timeline() {
    let mut test_context = get_test_context();

    let timeline = Timeline::new_sequence("existing_timeline");

    test_context.log_entity("some_entity".into(), |builder| {
        builder
            .with_archetype(RowId::new(), [(timeline, 0)], &Scalars::one(10.0))
            .with_archetype(RowId::new(), [(timeline, 1)], &Scalars::one(20.0))
            .with_archetype(RowId::new(), [(timeline, 2)], &Scalars::one(30.0))
    });

    let view_id = setup_blueprint(&mut test_context, &TimelineName::from("unknown_timeline"));

    run_view_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "unknown_timeline_view_ui",
        egui::vec2(300.0, 150.0),
    );

    run_view_selection_panel_ui_and_save_snapshot(
        &mut test_context,
        view_id,
        "unknown_timeline_selection_panel_ui",
        egui::vec2(300.0, 450.0),
    );
}

fn get_test_context() -> TestContext {
    let mut test_context = TestContext::default();

    // It's important to first register the view class before adding any entities,
    // otherwise the `VisualizerEntitySubscriber` for our visualizers doesn't exist yet,
    // and thus will not find anything applicable to the visualizer.
    test_context.register_view_class::<re_view_dataframe::DataframeView>();

    // Make sure we can draw stuff in the table.
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();

    test_context
}

fn setup_blueprint(test_context: &mut TestContext, timeline_name: &TimelineName) -> ViewId {
    test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let view_blueprint = ViewBlueprint::new(
            re_view_dataframe::DataframeView::identifier(),
            RecommendedView::root(),
        );

        let view_id = view_blueprint.id;
        blueprint.add_views(std::iter::once(view_blueprint), None, None);

        // the dataframe view to the desired timeline
        let query = re_view_dataframe::Query::from_blueprint(ctx, view_id);
        query.save_timeline_name(ctx, timeline_name);

        view_id
    })
}

fn run_view_ui_and_save_snapshot(
    test_context: &mut TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            re_ui::apply_style_and_install_loaders(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                test_context.run(ctx, |ctx| {
                    let view_class = ctx
                        .view_class_registry()
                        .get_class_or_log_error(DataframeView::identifier());

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
                        .expect("failed to run graph view ui");
                });

                test_context.handle_system_commands();
            });
        });

    harness.run();
    harness.snapshot(name);
}

fn run_view_selection_panel_ui_and_save_snapshot(
    test_context: &mut TestContext,
    view_id: ViewId,
    name: &str,
    size: egui::Vec2,
) {
    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            re_ui::apply_style_and_install_loaders(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                test_context.run(ctx, |ctx| {
                    let view_class = ctx
                        .view_class_registry()
                        .get_class_or_log_error(DataframeView::identifier());

                    let view_blueprint = ViewBlueprint::try_from_db(
                        view_id,
                        ctx.store_context.blueprint,
                        ctx.blueprint_query,
                    )
                    .expect("we just created that view");

                    let spacing = ui.spacing().item_spacing;
                    ui.list_item_scope("test_harness", |ui| {
                        ui.spacing_mut().item_spacing = spacing;

                        let mut view_states = test_context.view_states.lock();
                        let view_state = view_states.get_mut_or_create(view_id, view_class);

                        view_class
                            .selection_ui(
                                ctx,
                                ui,
                                view_state,
                                &view_blueprint.space_origin,
                                view_id,
                            )
                            .expect("failed to run view selection panel ui");
                    });
                });

                test_context.handle_system_commands();
            });
        });

    harness.run();
    harness.snapshot(name);
}
