use rayon::prelude::*;

use re_viewer_context::{
    SpaceViewClassIdentifier, SpaceViewSystemRegistry, SystemExecutionOutput, ViewQuery,
    ViewerContext,
};

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
