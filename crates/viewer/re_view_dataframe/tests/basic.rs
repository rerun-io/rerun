use re_chunk_store::RowId;
use re_log_types::{Timeline, TimelineName};
use re_types::archetypes::Scalar;
use re_view_dataframe::DataframeView;
use re_viewer_context::test_context::TestContext;
use re_viewer_context::{RecommendedView, ViewClass};
use re_viewport_blueprint::test_context_ext::TestContextExt;
use re_viewport_blueprint::ViewBlueprint;

#[test]
pub fn test_null_timeline() {
    let mut test_context = get_test_context();

    let timeline_a = Timeline::new_sequence("timeline_a");
    let timeline_b = Timeline::new_sequence("timeline_b");

    test_context.log_entity("first".into(), |builder| {
        builder.with_archetype(RowId::new(), [(timeline_a, 0)], &Scalar::new(10.0))
    });

    test_context.log_entity("second".into(), |builder| {
        builder.with_archetype(
            RowId::new(),
            [(timeline_a, 1), (timeline_b, 10)],
            &Scalar::new(12.0),
        )
    });

    run_graph_view_and_save_snapshot(
        test_context,
        timeline_a.name(),
        "null_timeline",
        egui::vec2(300.0, 150.0),
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

//TODO(ab): this utility could likely be generalized for all view tests
fn run_graph_view_and_save_snapshot(
    mut test_context: TestContext,
    timeline_name: &TimelineName,
    name: &str,
    size: egui::Vec2,
) {
    let view_id = test_context.setup_viewport_blueprint(|ctx, blueprint| {
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
    });

    let mut view_state = test_context
        .view_class_registry
        .get_class_or_log_error(DataframeView::identifier())
        .new_state();

    let mut harness = test_context
        .setup_kittest_for_rendering()
        .with_size(size)
        .build(|ctx| {
            re_ui::apply_style_and_install_loaders(ctx);

            egui::CentralPanel::default().show(ctx, |ui| {
                test_context.run(ctx, |ctx| {
                    let view_class = ctx
                        .view_class_registry
                        .get_class_or_log_error(DataframeView::identifier());

                    let view_blueprint = ViewBlueprint::try_from_db(
                        view_id,
                        ctx.store_context.blueprint,
                        ctx.blueprint_query,
                    )
                    .expect("we just created that view");

                    let (view_query, system_execution_output) =
                        re_viewport::execute_systems_for_view(
                            ctx,
                            &view_blueprint,
                            ctx.current_query().at(), // TODO(andreas): why is this even needed to be passed in?
                            &*view_state,
                        );

                    view_class
                        .ui(
                            ctx,
                            ui,
                            &mut *view_state,
                            &view_query,
                            system_execution_output,
                        )
                        .expect("failed to run graph view ui");
                });

                test_context.handle_system_commands();
            });
        });

    harness.run();
    harness.snapshot(name);
}
