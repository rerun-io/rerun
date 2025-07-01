use ahash::HashMap;

use re_viewer_context::{
    Contents, ViewId, ViewerContext, VisitorControlFlow, test_context::TestContext,
};

use re_viewport_blueprint::{DataQueryPropertyResolver, ViewBlueprint, ViewportBlueprint};

use crate::execute_systems_for_view;

/// Extension trait to [`TestContext`] for blueprint-related features.
pub trait TestContextExt {
    /// See docstring on the implementation below.
    fn setup_viewport_blueprint<R>(
        &mut self,
        setup_blueprint: impl FnOnce(&ViewerContext<'_>, &mut ViewportBlueprint) -> R,
    ) -> R;

    /// Displays the UI for a single given view.
    fn ui_for_single_view(&self, ui: &mut egui::Ui, ctx: &ViewerContext<'_>, view_id: ViewId);

    /// [`TestContext::run`] inside a central panel that displays the ui for a single given view.
    fn run_with_single_view(&mut self, ctx: &egui::Context, view_id: ViewId);
}

impl TestContextExt for TestContext {
    /// Inspect or update the blueprint of a [`TestContext`].
    ///
    /// This helper works by deserializing the current blueprint, providing it to the provided
    /// closure, and saving it back to the blueprint store. The closure should call the appropriate
    /// methods of [`ViewportBlueprint`] to inspect and/or create views and containers as required.
    ///
    /// Each time [`TestContextExt::setup_viewport_blueprint`] is called, it entirely recomputes the
    /// "query results", i.e., the [`re_viewer_context::DataResult`]s that each view contains, based
    /// on the current content of the recording store.
    ///
    /// Important pre-requisite:
    /// - The current timeline must already be set to the timeline of interest, because some
    ///   updates are timeline-dependant (in particular those related to visible time rane).
    /// - The view classes used by view must be already registered (see
    ///   [`TestContext::register_view_class`]).
    /// - The data store must be already populated for the views to have any content (see, e.g.,
    ///   [`TestContext::log_entity`]).
    ///
    fn setup_viewport_blueprint<R>(
        &mut self,
        setup_blueprint: impl FnOnce(&ViewerContext<'_>, &mut ViewportBlueprint) -> R,
    ) -> R {
        let mut setup_blueprint: Option<_> = Some(setup_blueprint);

        let mut result = None;

        egui::__run_test_ctx(|egui_ctx| {
            // We use `take` to ensure that the blueprint is setup only once, since egui forces
            // us to a `FnMut` closure.
            if let Some(setup_blueprint) = setup_blueprint.take() {
                self.run(egui_ctx, |ctx| {
                    let mut viewport_blueprint =
                        ViewportBlueprint::from_db(&self.blueprint_store, &self.blueprint_query);
                    result = Some(setup_blueprint(ctx, &mut viewport_blueprint));
                    viewport_blueprint.save_to_blueprint_store(ctx);
                });

                self.handle_system_commands();

                // Reload the blueprint store and execute all view queries.
                let viewport_blueprint =
                    ViewportBlueprint::from_db(&self.blueprint_store, &self.blueprint_query);

                let mut query_results = HashMap::default();

                self.run(egui_ctx, |ctx| {
                    viewport_blueprint.visit_contents::<()>(&mut |contents, _| {
                        if let Contents::View(view_id) = contents {
                            let view_blueprint = viewport_blueprint
                                .view(view_id)
                                .expect("view is known to exist");
                            let class_identifier = view_blueprint.class_identifier();

                            let visualizable_entities = ctx
                                .view_class_registry()
                                .class(class_identifier)
                                .unwrap_or_else(|| panic!("The class '{class_identifier}' must be registered beforehand"))
                                .determine_visualizable_entities(
                                    ctx.maybe_visualizable_entities_per_visualizer,
                                    ctx.recording(),
                                    &ctx.view_class_registry()
                                        .new_visualizer_collection(class_identifier),
                                    &view_blueprint.space_origin,
                                );

                            let indicated_entities_per_visualizer = ctx
                                .view_class_registry()
                                .indicated_entities_per_visualizer(&ctx.recording().store_id());

                            let mut data_query_result = view_blueprint.contents.execute_query(
                                ctx.store_context,
                                ctx.view_class_registry(),
                                ctx.blueprint_query,
                                &visualizable_entities,
                            );

                            let resolver = DataQueryPropertyResolver::new(
                                view_blueprint,
                                ctx.view_class_registry(),
                                ctx.maybe_visualizable_entities_per_visualizer,
                                &visualizable_entities,
                                &indicated_entities_per_visualizer,
                            );

                            resolver.update_overrides(
                                ctx.store_context.blueprint,
                                ctx.blueprint_query,
                                ctx.rec_cfg.time_ctrl.read().timeline(),
                                ctx.view_class_registry(),
                                &mut data_query_result,
                                &mut self.view_states.lock(),
                            );

                            query_results.insert(*view_id, data_query_result);
                        }

                        VisitorControlFlow::Continue
                    });
                });

                self.query_results = query_results;
            }
        });

        result.expect("The `setup_closure` is expected to be called at least once")
    }

    /// Displays the UI for a single given view.
    fn ui_for_single_view(&self, ui: &mut egui::Ui, ctx: &ViewerContext<'_>, view_id: ViewId) {
        let view_blueprint =
            ViewBlueprint::try_from_db(view_id, ctx.store_context.blueprint, ctx.blueprint_query)
                .expect("expected the view id to be known to the blueprint store");

        let view_class = ctx
            .view_class_registry()
            .get_class_or_log_error(view_blueprint.class_identifier());

        let mut view_states = self.view_states.lock();
        let view_state = view_states.get_mut_or_create(view_id, view_class);

        let (view_query, system_execution_output) =
            execute_systems_for_view(ctx, &view_blueprint, view_state);

        view_class
            .ui(ctx, ui, view_state, &view_query, system_execution_output)
            .expect("failed to run view ui");
    }

    /// [`TestContext::run`] inside a central panel that displays the ui for a single given view.
    fn run_with_single_view(&mut self, ctx: &egui::Context, view_id: ViewId) {
        // This is also called by `TestContext::run`,  but since it may change offsets in the central panel,
        // we have to call it before creating any ui.
        re_ui::apply_style_and_install_loaders(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            self.run(ctx, |ctx| {
                self.ui_for_single_view(ui, ctx, view_id);
            });

            self.handle_system_commands();
        });
    }
}
