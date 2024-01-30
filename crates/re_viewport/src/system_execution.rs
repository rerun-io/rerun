use std::collections::BTreeMap;

use ahash::HashMap;
use rayon::prelude::*;

use re_viewer_context::{
    SpaceViewClassIdentifier, SpaceViewId, SystemExecutionOutput, ViewQuery, ViewerContext,
};

use crate::{space_view_highlights::highlights_for_space_view, SpaceViewBlueprint};

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

pub fn execute_systems_for_space_views<'a>(
    ctx: &'a ViewerContext<'a>,
    tree: &egui_tiles::Tree<SpaceViewId>,
    space_views: &'a BTreeMap<SpaceViewId, SpaceViewBlueprint>,
) -> HashMap<SpaceViewId, (ViewQuery<'a>, SystemExecutionOutput)> {
    let Some(time_int) = ctx.rec_cfg.time_ctrl.read().time_int() else {
        return HashMap::default();
    };

    re_tracing::profile_wait!("execute_systems");

    tree.active_tiles()
        .into_iter()
        .filter_map(|tile_id| {
            tree.tiles.get(tile_id).and_then(|tile| match tile {
                egui_tiles::Tile::Pane(space_view_id) => {
                    space_views.get(space_view_id).map(|space_view_blueprint| {
                        let highlights = highlights_for_space_view(ctx, *space_view_id);
                        let output =
                            space_view_blueprint.execute_systems(ctx, time_int, highlights);
                        (*space_view_id, output)
                    })
                }
                egui_tiles::Tile::Container(_) => None,
            })
        })
        .collect::<HashMap<_, _>>()
}
