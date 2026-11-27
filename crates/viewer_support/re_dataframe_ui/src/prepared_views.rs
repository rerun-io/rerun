//! Runs views from a blueprint against a recording, independent of the main viewport.

use std::sync::Arc;

use ahash::HashMap as AHashMap;
use itertools::Itertools as _;
use nohash_hasher::{IntMap, IntSet};

use re_chunk_store::LatestAtQuery;
use re_log_types::StoreId;
use re_sdk_types::ViewClassIdentifier;
use re_view::execute_systems_for_view;
use re_viewer_context::{
    ActiveStoreContext, AppContext, ApplicationSelectionState, DataQueryResult, IndicatedEntities,
    MissingChunkReporter, PerVisualizerType, TimeControl, ViewClass,
    ViewContextSystemOncePerFrameResult, ViewId, ViewStates, ViewSystemIdentifier, ViewerContext,
    VisualizableEntities,
};
use re_viewport_blueprint::ViewBlueprint;

/// Once-per-frame context-system results of the view classes covered so far.
type OncePerFrameResults = IntMap<ViewSystemIdentifier, ViewContextSystemOncePerFrameResult>;

/// Once-per-frame context-system results for each recording and time query.
///
/// Each [`PreparedViews::new`] runs the context systems of the view classes that the cache doesn't
/// cover yet for its recording and the current query of its time control.
/// Only share one cache between `PreparedViews::new` calls within one UI frame.
#[derive(Default)]
pub struct OncePerFrameCache {
    per_query: AHashMap<(StoreId, LatestAtQuery), CachedOncePerFrameResults>,
}

#[derive(Default)]
struct CachedOncePerFrameResults {
    /// View classes whose context systems have results in `results`.
    view_classes: IntSet<ViewClassIdentifier>,

    results: OncePerFrameResults,
}

impl OncePerFrameCache {
    /// Returns results that cover the context systems of `view_classes` for the recording and the
    /// current query of `ctx`.
    fn results(
        &mut self,
        ctx: &ViewerContext<'_>,
        view_classes: impl IntoIterator<Item = ViewClassIdentifier>,
    ) -> &OncePerFrameResults {
        let cached = self
            .per_query
            .entry((ctx.recording().store_id().clone(), ctx.current_query()))
            .or_default();

        let missing_view_classes = view_classes
            .into_iter()
            .filter(|view_class| !cached.view_classes.contains(view_class))
            .unique()
            .collect_vec();
        if !missing_view_classes.is_empty() {
            let new_results = ctx
                .view_class_registry()
                .run_once_per_frame_context_systems(ctx, missing_view_classes.iter().copied());
            cached.results.extend(new_results);
            cached.view_classes.extend(missing_view_classes);
        }

        &cached.results
    }
}

/// Whether views shown by a [`PreparedViews`] get input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewInteraction {
    /// The views get input, use the selection of the app and errors from their UI are logged.
    #[expect(dead_code, reason = "no caller constructs it")]
    Interactive,

    /// The views get almost no input, their UI is disabled, they have an empty selection and
    /// errors from their UI are not logged.
    Passive,
}

/// A view from the blueprint, with its class.
struct ResolvedView<'a> {
    view_id: ViewId,
    view_blueprint: ViewBlueprint,
    view_class: &'a dyn ViewClass,
}

/// Stores and query results that a [`ViewerContext`] for [`PreparedViews`] borrows.
struct ViewerContextParts<'a> {
    app_ctx: AppContext<'a>,
    store_context: ActiveStoreContext<'a>,
    visualizable_entities_per_visualizer: PerVisualizerType<&'a VisualizableEntities>,
    indicated_entities_per_visualizer: PerVisualizerType<&'a IndicatedEntities>,
    query_results: AHashMap<ViewId, Arc<DataQueryResult>>,
    blueprint_time_ctrl: TimeControl,
    blueprint_query: &'a LatestAtQuery,
    interaction: ViewInteraction,
    empty_selection_state: ApplicationSelectionState,
}

impl ViewerContextParts<'_> {
    /// One shared `ViewerContext` for all views of this recording.
    fn ctx(&self) -> ViewerContext<'_> {
        ViewerContext {
            app_ctx: AppContext {
                active_store_context: Some(&self.store_context),
                selection_state: match self.interaction {
                    ViewInteraction::Interactive => self.app_ctx.selection_state,
                    ViewInteraction::Passive => &self.empty_selection_state,
                },
                ..self.app_ctx.clone()
            },
            store_context: &self.store_context,
            visualizable_entities_per_visualizer: &self.visualizable_entities_per_visualizer,
            indicated_entities_per_visualizer: &self.indicated_entities_per_visualizer,
            query_results: &self.query_results,
            time_ctrl: self.store_context.time_ctrl,
            blueprint_time_ctrl: &self.blueprint_time_ctrl,
            blueprint_query: self.blueprint_query,
        }
    }
}

/// Views that are ready to be shown, returned by [`PreparedViews::new`].
pub struct PreparedViews<'a> {
    parts: ViewerContextParts<'a>,

    /// The views of the `view_ids` given to [`PreparedViews::new`] that are in the blueprint.
    views: Vec<ResolvedView<'a>>,

    once_per_frame_results: &'a OncePerFrameResults,
}

impl<'a> PreparedViews<'a> {
    /// Prepares the views in `view_ids` and creates their states in `view_states`.
    ///
    /// The views see the stores of `store_context` as the active stores.
    /// See [`OncePerFrameCache`] for when the once-per-frame context systems run.
    pub fn new(
        app_ctx: &'a AppContext<'a>,
        store_context: ActiveStoreContext<'a>,
        blueprint_query: &'a LatestAtQuery,
        view_ids: &[ViewId],
        view_states: &mut ViewStates,
        once_per_frame_cache: &'a mut OncePerFrameCache,
        interaction: ViewInteraction,
    ) -> Self {
        re_tracing::profile_function!();

        let view_class_registry = app_ctx.view_class_registry;

        // Derive visualizable/indicated entities from the bootstrapped cache.
        let visualizable_entities_per_visualizer = store_context
            .caches
            .visualizable_entities_for_visualizer_systems();
        let indicated_entities_per_visualizer =
            store_context.caches.indicated_entities_per_visualizer();

        let store_id = store_context.recording.store_id();

        // Resolve each view's blueprint + class once.
        let views: Vec<ResolvedView<'_>> = view_ids
            .iter()
            .unique()
            .filter_map(|view_id| {
                let view_blueprint =
                    ViewBlueprint::try_from_db(*view_id, store_context.blueprint, blueprint_query)?;
                let view_class = view_blueprint.class(view_class_registry);
                Some(ResolvedView {
                    view_id: *view_id,
                    view_blueprint,
                    view_class,
                })
            })
            .collect();

        // Build the per-view data-result trees against the shared store context so that the
        // `ViewerContext` can expose results for all views at once.
        for view in &views {
            view_states.get_mut_or_create(store_id, view.view_id, view.view_class);
        }
        let query_results = re_viewport_blueprint::query_results_for_views(
            &store_context,
            app_ctx.app_caches,
            views.iter().map(|view| &view.view_blueprint),
            view_class_registry,
            blueprint_query,
            view_states,
            &visualizable_entities_per_visualizer,
            &indicated_entities_per_visualizer,
        );

        let parts = ViewerContextParts {
            app_ctx: app_ctx.clone(),
            store_context,
            visualizable_entities_per_visualizer,
            indicated_entities_per_visualizer,
            query_results,
            blueprint_time_ctrl: TimeControl::default(),
            blueprint_query,
            interaction,
            empty_selection_state: ApplicationSelectionState::default(),
        };

        let once_per_frame_results = once_per_frame_cache.results(
            &parts.ctx(),
            views
                .iter()
                .map(|view| view.view_blueprint.class_identifier()),
        );

        Self {
            parts,
            views,
            once_per_frame_results,
        }
    }

    fn view(&self, view_id: ViewId) -> Option<&ResolvedView<'a>> {
        self.views.iter().find(|view| view.view_id == view_id)
    }

    /// Returns the class of the view, or `None` if the view is missing from the blueprint.
    pub fn view_class(&self, view_id: ViewId) -> Option<&'a dyn ViewClass> {
        self.view(view_id).map(|view| view.view_class)
    }

    /// Shows the view in the remaining space of `ui`.
    ///
    /// Shows a loading indicator over the view while chunks are missing.
    /// Does nothing if the view is missing from the blueprint.
    /// `id_salt` must be unique for each view shown under `ui`.
    pub fn show_view(
        &self,
        ui: &mut egui::Ui,
        view_id: ViewId,
        view_states: &mut ViewStates,
        id_salt: impl egui::AsIdSalt + Copy,
    ) {
        let Some(ResolvedView {
            view_id,
            view_blueprint,
            view_class,
        }) = self.view(view_id)
        else {
            return;
        };
        let ctx = self.parts.ctx();
        let recording = ctx.recording();
        let store_id = recording.store_id();

        let view_state = view_states.get_mut_or_create(store_id, *view_id, *view_class);
        let (view_query, system_execution_output) = execute_systems_for_view(
            &ctx,
            view_blueprint,
            view_state,
            self.once_per_frame_results,
        );

        let missing_chunk_reporter =
            MissingChunkReporter::new(system_execution_output.any_missing_chunks());

        let view_state = view_states.get_mut_or_create(store_id, *view_id, *view_class);

        let view_rect = ui.available_rect_before_wrap();

        let passive = match self.parts.interaction {
            ViewInteraction::Interactive => false,
            ViewInteraction::Passive => true,
        };

        // Suppress most input while rendering passive views.
        let input_before = passive.then(|| {
            ui.input_mut(|input| {
                let input_before = input.clone();

                input.modifiers = egui::Modifiers::default();
                input.raw.events.clear();
                input.smooth_scroll_delta = egui::Vec2::ZERO;
                input.focused = false;
                input.keys_down.clear();
                input.pointer = egui::PointerState::default();

                input_before
            })
        });
        let result = ui.push_id(id_salt, |ui| {
            if passive {
                ui.disable();
            }
            view_class.ui(
                &ctx,
                &missing_chunk_reporter,
                ui,
                view_state,
                &view_query,
                system_execution_output,
            )
        });
        if let Some(input_before) = input_before {
            ui.input_mut(|input| {
                *input = input_before;
            });
        }
        if !passive && let Err(err) = result.inner {
            re_log::error_once!("Error showing view {view_id}: {err}");
        }

        re_view::paint_view_loading_indicator(
            ui,
            id_salt,
            view_rect,
            missing_chunk_reporter.any_missing(),
            recording,
        );
    }
}
