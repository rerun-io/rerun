//! Extends the `re_test_context` with viewport-related features.

mod test_view;

use ahash::HashMap;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::{SnapshotOptions, SnapshotResult};
use re_viewer_context::{Contents, ViewId, ViewerContext, VisitorControlFlow};
use re_viewport::execute_systems_for_view;
use re_viewport_blueprint::{DataQueryPropertyResolver, ViewBlueprint, ViewportBlueprint};
pub use test_view::TestView;

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
    fn run_with_single_view(&self, ui: &mut egui::Ui, view_id: ViewId);

    fn run_view_ui_and_save_snapshot(
        &self,
        view_id: ViewId,
        snapshot_name: &str,
        size: egui::Vec2,
        snapshot_options: Option<SnapshotOptions>,
    ) -> SnapshotResult;
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
    ///   updates are timeline-dependant (in particular those related to visible time range).
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
                        ViewportBlueprint::from_db(ctx.blueprint_db(), &self.blueprint_query);
                    result = Some(setup_blueprint(ctx, &mut viewport_blueprint));
                    viewport_blueprint.save_to_blueprint_store(ctx);
                });

                self.handle_system_commands(egui_ctx);

                // Reload the blueprint store and execute all view queries.
                let blueprint_query = self.blueprint_query.clone();
                let viewport_blueprint =
                    ViewportBlueprint::from_db(self.active_blueprint(), &blueprint_query);

                let mut query_results = HashMap::default();

                self.run(egui_ctx, |ctx| {
                    let _ignored = viewport_blueprint.visit_contents::<()>(&mut |contents, _| {
                        if let Contents::View(view_id) = contents {
                            let view_blueprint = viewport_blueprint
                                .view(view_id)
                                .expect("view is known to exist");

                            let class_registry = ctx.view_class_registry();
                            let class_identifier = view_blueprint.class_identifier();
                            let class = class_registry.class(class_identifier).unwrap_or_else(|| panic!("The class '{class_identifier}' must be registered beforehand"));

                            let visualizable_entities_for_view = ctx.collect_visualizable_entities_for_view_class(class_identifier);

                            let mut data_query_result = view_blueprint.contents.build_data_result_tree(
                                ctx.store_context,
                                class_registry,
                                ctx.blueprint_query,
                                &visualizable_entities_for_view,
                            );

                            let resolver = DataQueryPropertyResolver::new(
                                view_blueprint,
                                class_registry,
                                &visualizable_entities_for_view,
                                ctx.indicated_entities_per_visualizer,
                            );

                            resolver.update_overrides(
                                ctx.store_context.blueprint,
                                ctx.blueprint_query,
                                ctx.time_ctrl.timeline(),
                                class_registry,
                                &mut data_query_result,
                                self.view_states.lock().get_mut_or_create(*view_id, class),
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

        let class_registry = ctx.view_class_registry();
        let class_identifier = view_blueprint.class_identifier();
        let view_class = class_registry.get_class_or_log_error(class_identifier);

        let mut view_states = self.view_states.lock();
        view_states.reset_visualizer_errors();
        let view_state = view_states.get_mut_or_create(view_id, view_class);

        let context_system_once_per_frame_results = class_registry
            .run_once_per_frame_context_systems(ctx, std::iter::once(class_identifier));
        let (view_query, system_execution_output) = execute_systems_for_view(
            ctx,
            &view_blueprint,
            view_state,
            &context_system_once_per_frame_results,
        );
        view_states.report_visualizer_errors(view_id, &system_execution_output);

        let view_state = view_states.get_mut_or_create(view_id, view_class);
        view_class
            .ui(ctx, ui, view_state, &view_query, system_execution_output)
            .expect("failed to run view ui");
    }

    /// [`TestContext::run`] for a single view.
    fn run_with_single_view(&self, ui: &mut egui::Ui, view_id: ViewId) {
        self.run_ui(ui, |ctx, ui| {
            self.ui_for_single_view(ui, ctx, view_id);
        });

        self.handle_system_commands(ui.ctx());
    }

    fn run_view_ui_and_save_snapshot(
        &self,
        view_id: ViewId,
        snapshot_name: &str,
        size: egui::Vec2,
        snapshot_options: Option<SnapshotOptions>,
    ) -> SnapshotResult {
        let mut harness = self.setup_kittest_for_rendering_3d(size).build_ui(|ui| {
            self.run_with_single_view(ui, view_id);
        });
        harness.run();

        if let Some(snapshot_options) = snapshot_options {
            harness.try_snapshot_options(snapshot_name, &snapshot_options)
        } else {
            harness.try_snapshot(snapshot_name)
        }
    }
}
