use std::collections::BTreeMap;

use ahash::HashMap;
use rayon::prelude::*;

use re_log_types::TimeInt;
use re_viewer_context::{
    PerSystemDataResults, SpaceViewClassIdentifier, SpaceViewHighlights, SpaceViewId,
    SystemExecutionOutput, ViewQuery, ViewerContext,
};

use crate::space_view_highlights::highlights_for_space_view;
use re_space_view::SpaceViewBlueprint;

pub fn create_and_run_space_view_systems(
    ctx: &ViewerContext<'_>,
    space_view_class: SpaceViewClassIdentifier,
    query: &ViewQuery<'_>,
) -> SystemExecutionOutput {
    re_tracing::profile_function!(space_view_class.as_str());

    let context_systems = {
        re_tracing::profile_wait!("ViewContextSystem::execute");
        let mut context_systems = ctx
            .space_view_class_registry
            .new_context_collection(space_view_class);
        context_systems
            .systems
            .par_iter_mut()
            .for_each(|(_name, system)| {
                re_tracing::profile_scope!("ViewContextSystem::execute", _name.as_str());
                system.execute(ctx, query);
            });
        context_systems
    };

    re_tracing::profile_wait!("VisualizerSystem::execute");
    let mut view_systems = ctx
        .space_view_class_registry
        .new_visualizer_collection(space_view_class);
    let draw_data = view_systems
        .systems
        .par_iter_mut()
        .map(|(name, part)| {
            re_tracing::profile_scope!("VisualizerSystem::execute", name.as_str());
            match part.execute(ctx, query, &context_systems) {
                Ok(part_draw_data) => part_draw_data,
                Err(err) => {
                    re_log::error_once!("Error executing visualizer {name:?}: {err}");
                    Vec::new()
                }
            }
        })
        .flatten()
        .collect();

    SystemExecutionOutput {
        view_systems,
        context_systems,
        draw_data,
    }
}

pub fn execute_systems_for_all_space_views<'a>(
    ctx: &'a ViewerContext<'a>,
    tree: &egui_tiles::Tree<SpaceViewId>,
    space_views: &'a BTreeMap<SpaceViewId, SpaceViewBlueprint>,
) -> HashMap<SpaceViewId, (ViewQuery<'a>, SystemExecutionOutput)> {
    let Some(time_int) = ctx.rec_cfg.time_ctrl.read().time_int() else {
        return HashMap::default();
    };

    re_tracing::profile_wait!("execute_systems");

    tree.active_tiles()
        .into_par_iter()
        .filter_map(|tile_id| {
            tree.tiles.get(tile_id).and_then(|tile| match tile {
                egui_tiles::Tile::Pane(space_view_id) => {
                    space_views.get(space_view_id).map(|space_view_blueprint| {
                        let highlights = highlights_for_space_view(ctx, *space_view_id);
                        let output = execute_systems_for_space_view(
                            ctx,
                            space_view_blueprint,
                            time_int,
                            highlights,
                        );
                        (*space_view_id, output)
                    })
                }
                egui_tiles::Tile::Container(_) => None,
            })
        })
        .collect::<HashMap<_, _>>()
}

pub fn execute_systems_for_space_view<'a>(
    ctx: &'a ViewerContext<'_>,
    space_view: &'a SpaceViewBlueprint,
    latest_at: TimeInt,
    highlights: SpaceViewHighlights,
) -> (ViewQuery<'a>, SystemExecutionOutput) {
    re_tracing::profile_function!(space_view.class_identifier().as_str());

    let query_result = ctx.lookup_query_result(space_view.id);

    let mut per_system_data_results = PerSystemDataResults::default();
    {
        re_tracing::profile_scope!("per_system_data_results");

        query_result.tree.visit(&mut |node| {
            for system in &node.data_result.visualizers {
                per_system_data_results
                    .entry(*system)
                    .or_default()
                    .push(&node.data_result);
            }
            true
        });
    }

    let query = re_viewer_context::ViewQuery {
        space_view_id: space_view.id,
        space_origin: &space_view.space_origin,
        per_system_data_results,
        timeline: *ctx.rec_cfg.time_ctrl.read().timeline(),
        latest_at,
        highlights,
    };

    let system_output =
        create_and_run_space_view_systems(ctx, *space_view.class_identifier(), &query);

    (query, system_output)
}
