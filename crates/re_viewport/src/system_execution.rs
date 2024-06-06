use std::collections::BTreeMap;

use ahash::HashMap;
use rayon::prelude::*;

use re_log_types::TimeInt;
use re_types::SpaceViewClassIdentifier;
use re_viewer_context::{
    PerSystemDataResults, SpaceViewId, SpaceViewState, SystemExecutionOutput,
    ViewContextCollection, ViewQuery, ViewStates, ViewerContext, VisualizerCollection,
};

use crate::space_view_highlights::highlights_for_space_view;
use re_viewport_blueprint::SpaceViewBlueprint;

fn run_space_view_systems(
    ctx: &ViewerContext<'_>,
    space_view_class: SpaceViewClassIdentifier,
    query: &ViewQuery<'_>,
    view_state: &dyn SpaceViewState,
    context_systems: &mut ViewContextCollection,
    view_systems: &mut VisualizerCollection,
) -> Vec<re_renderer::QueueableDrawData> {
    re_tracing::profile_function!(space_view_class.as_str());

    // TODO(jleibs): This is weird. Most of the time we don't need this.
    let visualizer_collection = ctx
        .space_view_class_registry
        .new_visualizer_collection(space_view_class);

    let view_ctx = re_viewer_context::ViewContext {
        viewer_ctx: ctx,
        view_id: query.space_view_id,
        view_state,
        visualizer_collection: &visualizer_collection,
    };

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

pub fn execute_systems_for_space_view<'a>(
    ctx: &'a ViewerContext<'_>,
    view: &'a SpaceViewBlueprint,
    latest_at: TimeInt,
    view_states: &ViewStates,
) -> (ViewQuery<'a>, SystemExecutionOutput) {
    re_tracing::profile_function!(view.class_identifier().as_str());

    let highlights = highlights_for_space_view(ctx, view.id);

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

    let query = re_viewer_context::ViewQuery {
        space_view_id: view.id,
        space_origin: &view.space_origin,
        per_visualizer_data_results,
        timeline: *ctx.rec_cfg.time_ctrl.read().timeline(),
        latest_at,
        highlights,
    };

    let mut context_systems = ctx
        .space_view_class_registry
        .new_context_collection(view.class_identifier());
    let mut view_systems = ctx
        .space_view_class_registry
        .new_visualizer_collection(view.class_identifier());

    let draw_data = if let Some(view_state) = view_states.get(view.id) {
        run_space_view_systems(
            ctx,
            view.class_identifier(),
            &query,
            view_state.view_state.as_ref(),
            &mut context_systems,
            &mut view_systems,
        )
    } else {
        re_log::error_once!("No view state found for view {}", view.id);
        Vec::new()
    };

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
    tree: &egui_tiles::Tree<SpaceViewId>,
    views: &'a BTreeMap<SpaceViewId, SpaceViewBlueprint>,
    view_states: &ViewStates,
) -> HashMap<SpaceViewId, (ViewQuery<'a>, SystemExecutionOutput)> {
    let Some(time_int) = ctx.rec_cfg.time_ctrl.read().time_int() else {
        return Default::default();
    };

    re_tracing::profile_wait!("execute_systems");

    tree.active_tiles()
        .into_par_iter()
        .filter_map(|tile_id| {
            tree.tiles.get(tile_id).and_then(|tile| match tile {
                egui_tiles::Tile::Pane(view_id) => views.get(view_id).map(|view| {
                    let result = execute_systems_for_space_view(ctx, view, time_int, view_states);

                    (*view_id, result)
                }),
                egui_tiles::Tile::Container(_) => None,
            })
        })
        .collect::<HashMap<_, _>>()
}
