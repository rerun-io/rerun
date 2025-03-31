use ahash::HashMap;

use re_viewer_context::{
    test_context::TestContext, Contents, ViewClassExt as _, ViewerContext, VisitorControlFlow,
};

use crate::{view_contents::DataQueryPropertyResolver, ViewportBlueprint};

/// Extension trait to [`TestContext`] for blueprint-related features.
pub trait TestContextExt {
    /// See docstring on the implementation below.
    fn setup_viewport_blueprint<R>(
        &mut self,
        setup_blueprint: impl FnOnce(&ViewerContext<'_>, &mut ViewportBlueprint) -> R,
    ) -> R;
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
                    let mut viewport_blueprint = ViewportBlueprint::try_from_db(
                        &self.blueprint_store,
                        &self.blueprint_query,
                    );
                    result = Some(setup_blueprint(ctx, &mut viewport_blueprint));
                    viewport_blueprint.save_to_blueprint_store(ctx);
                });

                self.handle_system_commands();

                // Reload the blueprint store and execute all view queries.
                let viewport_blueprint =
                    ViewportBlueprint::try_from_db(&self.blueprint_store, &self.blueprint_query);

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
                                    &ctx.maybe_visualizable_entities_per_visualizer,
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
                                &ctx.maybe_visualizable_entities_per_visualizer,
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
}
