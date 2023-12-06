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
    let (draw_data_sender, draw_data_receiver) = std::sync::mpsc::channel();

    view_systems
        .systems
        .par_iter_mut()
        .for_each(|(name, part)| {
            re_tracing::profile_scope!(name.as_str());
            match part.execute(ctx, query, &context_systems) {
                Ok(part_draw_data) => {
                    draw_data_sender.send(part_draw_data).ok();
                }
                Err(err) => {
                    re_log::error_once!("Error executing view part system {name:?}: {err}");
                }
            }
        });

    let mut draw_data = Vec::new();
    while let Ok(mut part_draw_data) = draw_data_receiver.try_recv() {
        draw_data.append(&mut part_draw_data);
    }

    SystemExecutionOutput {
        view_systems,
        context_systems,
        draw_data,
    }
}
