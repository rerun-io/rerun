use std::collections::BTreeMap;

use ahash::HashMap;
use rayon::prelude::*;
use re_view::execute_systems_for_view;
use re_viewer_context::{SystemExecutionOutput, ViewId, ViewQuery, ViewStates, ViewerContext};
use re_viewport_blueprint::ViewBlueprint;

pub fn execute_systems_for_all_views<'a>(
    ctx: &'a ViewerContext<'a>,
    tree: &egui_tiles::Tree<ViewId>,
    views: &'a BTreeMap<ViewId, ViewBlueprint>,
    view_states: &mut ViewStates,
) -> HashMap<ViewId, (ViewQuery<'a>, SystemExecutionOutput)> {
    re_tracing::profile_wait!("execute_systems");

    let store_id = ctx.store_id();

    // During system execution we only have read access to the view states, so we need to ensure they exist ahead of time.
    for (view_id, view) in views {
        view_states.ensure_state_exists(store_id, *view_id, view.class(ctx.view_class_registry()));
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
                    let Some(view_state) = view_states.get(store_id, *view_id) else {
                        re_log::debug_panic!("View state for view {view_id:?} not found. That shouldn't be possible since we just ensured they exist above.");
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
