use std::collections::BTreeMap;

use ahash::HashMap;
use rayon::prelude::*;

use re_viewer_context::{
    PerSystemDataResults, SystemExecutionOutput, ViewContextCollection, ViewId, ViewQuery,
    ViewState, ViewStates, ViewerContext, VisualizerCollection,
};

use crate::view_highlights::highlights_for_view;
use re_viewport_blueprint::ViewBlueprint;

fn run_view_systems(
    ctx: &ViewerContext<'_>,
    view: &ViewBlueprint,
    query: &ViewQuery<'_>,
    view_state: &dyn ViewState,
    context_systems: &mut ViewContextCollection,
    view_systems: &mut VisualizerCollection,
) -> Vec<re_renderer::QueueableDrawData> {
    re_tracing::profile_function!(view.class_identifier().as_str());

    let view_ctx = view.bundle_context_with_state(ctx, view_state);

    {
        re_tracing::profile_wait!("ViewContextSystem::execute");
        context_systems
            .systems
            .par_iter_mut()
            .for_each(|(_name, system)| {
                re_tracing::profile_scope!("ViewContextSystem::execute", _name.as_str());
                system.execute(&view_ctx, query);
            });
    };

    re_tracing::profile_wait!("VisualizerSystem::execute");
    view_systems
        .systems
        .par_iter_mut()
        .map(|(name, part)| {
            re_tracing::profile_scope!("VisualizerSystem::execute", name.as_str());
            match part.execute(&view_ctx, query, context_systems) {
                Ok(part_draw_data) => part_draw_data,
                Err(err) => {
                    re_log::error_once!("Error executing visualizer {name:?}: {err}");
                    Vec::new()
                }
            }
        })
        .flatten()
        .collect()
}

pub fn execute_systems_for_view<'a>(
    ctx: &'a ViewerContext<'_>,
    view: &'a ViewBlueprint,
    view_state: &dyn ViewState,
) -> (ViewQuery<'a>, SystemExecutionOutput) {
    re_tracing::profile_function!(view.class_identifier().as_str());

    let highlights = highlights_for_view(ctx, view.id);

    let query_result = ctx.lookup_query_result(view.id);

    let mut per_visualizer_data_results = PerSystemDataResults::default();
    {
        re_tracing::profile_scope!("per_system_data_results");

        query_result.tree.visit(&mut |node| {
            for system in &node.data_result.visualizers {
                per_visualizer_data_results
                    .entry(*system)
                    .or_default()
                    .push(&node.data_result);
            }
            true
        });
    }

    let current_query = ctx.rec_cfg.time_ctrl.read().current_query();
    let query = re_viewer_context::ViewQuery {
        view_id: view.id,
        space_origin: &view.space_origin,
        per_visualizer_data_results,
        timeline: current_query.timeline(),
        latest_at: current_query.at(),
        highlights,
    };

    let mut context_systems = ctx
        .view_class_registry
        .new_context_collection(view.class_identifier());
    let mut view_systems = ctx
        .view_class_registry
        .new_visualizer_collection(view.class_identifier());

    let draw_data = run_view_systems(
        ctx,
        view,
        &query,
        view_state,
        &mut context_systems,
        &mut view_systems,
    );

    (
        query,
        SystemExecutionOutput {
            view_systems,
            context_systems,
            draw_data,
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
        view_states.ensure_state_exists(*view_id, view.class(ctx.view_class_registry));
    }

    tree.active_tiles()
        .into_par_iter()
        .filter_map(|tile_id| {
            tree.tiles.get(tile_id).and_then(|tile| match tile {
                egui_tiles::Tile::Pane(view_id) => views.get(view_id).and_then(|view| {
                    let Some(view_state) = view_states.get(*view_id) else {
                        debug_assert!(false, "View state for view {view_id:?} not found. That shouldn't be possible since we just ensured they exist above.");
                        return None;
                    };

                    let result = execute_systems_for_view(ctx, view, view_state);
                    Some((*view_id, result))
                }),
                egui_tiles::Tile::Container(_) => None,
            })
        })
        .collect::<HashMap<_, _>>()
}
