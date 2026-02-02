use std::collections::BTreeMap;
use std::sync::Arc;

use ahash::HashMap;
use nohash_hasher::IntMap;
use rayon::prelude::*;
use re_viewer_context::{
    PerSystemDataResults, PerVisualizerTypeInViewClass, SystemExecutionOutput,
    ViewContextCollection, ViewContextSystemOncePerFrameResult, ViewId, ViewQuery, ViewState,
    ViewStates, ViewSystemExecutionError, ViewSystemIdentifier, ViewerContext,
    VisualizerCollection, VisualizerExecutionOutput,
};
use re_viewport_blueprint::ViewBlueprint;

use crate::view_highlights::highlights_for_view;

fn run_view_systems(
    ctx: &ViewerContext<'_>,
    view: &ViewBlueprint,
    query: &ViewQuery<'_>,
    view_state: &dyn ViewState,
    context_system_once_per_frame_results: &IntMap<
        ViewSystemIdentifier,
        ViewContextSystemOncePerFrameResult,
    >,
    context_systems: &mut ViewContextCollection,
    view_systems: &mut VisualizerCollection,
) -> PerVisualizerTypeInViewClass<Result<VisualizerExecutionOutput, Arc<ViewSystemExecutionError>>>
{
    re_tracing::profile_function!(view.class_identifier().as_str());

    let view_ctx = view.bundle_context_with_state(ctx, view_state);

    {
        re_tracing::profile_wait!("ViewContextSystem::execute");
        context_systems
            .systems
            .par_iter_mut()
            .for_each(|(name, system)| {
                re_tracing::profile_scope!("ViewContextSystem::execute", name.as_str());
                let once_per_frame_result = context_system_once_per_frame_results
                    .get(name)
                    .expect("Context system execution result didn't occur");
                system.execute(&view_ctx, query, once_per_frame_result);
            });
    };

    re_tracing::profile_wait!("VisualizerSystem::execute");
    let per_visualizer_type_results = view_systems
        .systems
        .par_iter_mut()
        .map(|(name, part)| {
            re_tracing::profile_scope!("VisualizerSystem::execute", name.as_str());
            let result = part.execute(&view_ctx, query, context_systems);
            (*name, result.map_err(Arc::new))
        })
        .collect();

    PerVisualizerTypeInViewClass {
        view_class_identifier: view.class_identifier(),
        per_visualizer: per_visualizer_type_results,
    }
}

/// Creates a new [`ViewQuery`] for the given view.
pub fn new_view_query<'a>(ctx: &'a ViewerContext<'a>, view: &'a ViewBlueprint) -> ViewQuery<'a> {
    let highlights = highlights_for_view(ctx, view.id);

    let query_result = ctx.lookup_query_result(view.id);

    let mut per_visualizer_data_results = PerSystemDataResults::default();
    {
        re_tracing::profile_scope!("per_system_data_results");

        query_result.tree.visit(&mut |node| {
            for instruction in &node.data_result.visualizer_instructions {
                per_visualizer_data_results
                    .entry(instruction.visualizer_type)
                    .or_default()
                    .push(&node.data_result);
            }
            true
        });
    }

    let current_query = ctx.time_ctrl.current_query();
    re_viewer_context::ViewQuery {
        view_id: view.id,
        space_origin: &view.space_origin,
        per_visualizer_data_results,
        timeline: current_query.timeline(),
        latest_at: current_query.at(),
        highlights,
    }
}

pub fn execute_systems_for_view<'a>(
    ctx: &'a ViewerContext<'_>,
    view: &'a ViewBlueprint,
    view_state: &dyn ViewState,
    context_system_once_per_frame_results: &IntMap<
        ViewSystemIdentifier,
        ViewContextSystemOncePerFrameResult,
    >,
) -> (ViewQuery<'a>, SystemExecutionOutput) {
    re_tracing::profile_function!(view.class_identifier().as_str());

    let query = new_view_query(ctx, view);

    let mut context_systems = ctx
        .view_class_registry()
        .new_context_collection(view.class_identifier());
    let mut view_systems = ctx
        .view_class_registry()
        .new_visualizer_collection(view.class_identifier());

    let visualizer_execution_output = run_view_systems(
        ctx,
        view,
        &query,
        view_state,
        context_system_once_per_frame_results,
        &mut context_systems,
        &mut view_systems,
    );

    (
        query,
        SystemExecutionOutput {
            view_systems,
            context_systems,
            visualizer_execution_output,
        },
    )
}

pub fn execute_systems_for_all_views<'a>(
    ctx: &'a ViewerContext<'a>,
    tree: &egui_tiles::Tree<ViewId>,
    views: &'a BTreeMap<ViewId, ViewBlueprint>,
    view_states: &mut ViewStates,
) -> HashMap<ViewId, (ViewQuery<'a>, SystemExecutionOutput)> {
    re_tracing::profile_wait!("execute_systems");

    // During system execution we only have read access to the view states, so we need to ensure they exist ahead of time.
    for (view_id, view) in views {
        view_states.ensure_state_exists(*view_id, view.class(ctx.view_class_registry()));
    }

    // Once-per-frame context system execution.
    // The same context system class may be used by several view classes, so we have to do this before
    // running anything per-view.
    let context_system_once_per_frame_results = ctx
        .view_class_registry()
        .run_once_per_frame_context_systems(
            ctx,
            views.values().map(|view| view.class_identifier()),
        );

    tree.active_tiles()
        .into_par_iter()
        .filter_map(|tile_id| {
            let tile = tree.tiles.get(tile_id)?;
            match tile {
                egui_tiles::Tile::Pane(view_id) => {
                    let view = views.get(view_id)?;
                    let Some(view_state) = view_states.get(*view_id) else {
                        debug_assert!(false, "View state for view {view_id:?} not found. That shouldn't be possible since we just ensured they exist above.");
                        return None;
                    };

                    let result = execute_systems_for_view(ctx, view, view_state, &context_system_once_per_frame_results);
                    Some((*view_id, result))
                },
                egui_tiles::Tile::Container(_) => None,
            }
        })
        .collect::<HashMap<_, _>>()
}
