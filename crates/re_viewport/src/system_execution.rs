use std::collections::BTreeMap;

use ahash::HashMap;
use rayon::prelude::*;

use re_viewer_context::{
    SpaceViewClassIdentifier, SpaceViewId, SpaceViewSystemRegistry, SystemExecutionOutput,
    ViewQuery, ViewerContext,
};

use crate::{space_view_highlights::highlights_for_space_view, SpaceViewBlueprint};

pub fn create_and_run_space_view_systems(
    ctx: &ViewerContext<'_>,
    space_view_identifier: SpaceViewClassIdentifier,
    systems: &SpaceViewSystemRegistry,
    query: &ViewQuery<'_>,
) -> SystemExecutionOutput {
    re_tracing::profile_function!();

    let context_systems = {
        re_tracing::profile_scope!("ViewContextSystem::execute");
        let mut context_systems = systems.new_context_collection(space_view_identifier);
        context_systems
            .systems
            .par_iter_mut()
            .for_each(|(_name, system)| {
                re_tracing::profile_scope!(_name.as_str());
                system.execute(ctx, query);
            });
        context_systems
    };

    re_tracing::profile_scope!("ViewPartSystem::execute");
    let mut view_systems = systems.new_part_collection();
    let draw_data = view_systems
        .systems
        .par_iter_mut()
        .map(|(name, part)| {
            re_tracing::profile_scope!(name.as_str());
            match part.execute(ctx, query, &context_systems) {
                Ok(part_draw_data) => part_draw_data,
                Err(err) => {
                    re_log::error_once!("Error executing view part system {name:?}: {err}");
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
    mut space_views_to_execute: Vec<SpaceViewId>,
    space_views: &'a BTreeMap<SpaceViewId, SpaceViewBlueprint>,
) -> HashMap<SpaceViewId, (ViewQuery<'a>, SystemExecutionOutput)> {
    re_tracing::profile_function!();

    let Some(time_int) = ctx.rec_cfg.time_ctrl.read().time_int() else {
        return HashMap::default();
    };

    space_views_to_execute
        .par_drain(..)
        .filter_map(|space_view_id| {
            let space_view_blueprint = space_views.get(&space_view_id)?;
            let highlights =
                highlights_for_space_view(ctx.selection_state(), space_view_id, space_views);
            let output = space_view_blueprint.execute_systems(ctx, time_int, highlights);
            Some((space_view_id, output))
        })
        .collect::<HashMap<_, _>>()
}
