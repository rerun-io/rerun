use nohash_hasher::IntMap;
use rayon::prelude::*;
use re_viewer_context::{
    MissingChunkReporter, PerVisualizerTypeInViewClass, SystemExecutionOutput,
    ViewContextCollection, ViewContextSystemOncePerFrameResult, ViewQuery, ViewState,
    ViewSystemExecutionError, ViewSystemIdentifier, ViewSystemState, ViewerContext,
    VisualizerCollection, VisualizerExecutionOutput, VisualizerInstructionsPerType,
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
    view_systems: &VisualizerCollection,
) -> PerVisualizerTypeInViewClass<Result<VisualizerExecutionOutput, ViewSystemExecutionError>> {
    re_tracing::profile_function!(view.class_identifier().as_str());

    let view_ctx = view.bundle_context_with_state(ctx, view_state);

    {
        re_tracing::profile_wait!("ViewContextSystem::execute");
        context_systems
            .systems
            .par_iter_mut()
            .for_each(|(name, (view_ctx_system, state))| {
                re_tracing::profile_scope!("ViewContextSystem::execute", name.as_str());
                let missing_chunk_reporter = MissingChunkReporter::default();
                let once_per_frame_result = context_system_once_per_frame_results
                    .get(name)
                    .expect("Context system execution result didn't occur");
                view_ctx_system.execute(
                    &view_ctx,
                    &missing_chunk_reporter,
                    query,
                    once_per_frame_result,
                );
                *state = ViewSystemState {
                    any_missing_chunks: missing_chunk_reporter.any_missing(),
                };
            });
    };

    re_tracing::profile_wait!("VisualizerSystem::execute");
    let per_visualizer_type_results = view_systems
        .systems
        .par_iter()
        .map(|(name, vis_system)| {
            // Skip execution when no entities in the view have instructions for this
            // visualizer.
            if !query
                .active_visualizer_instructions_per_type
                .contains_key(name)
            {
                let mut output = VisualizerExecutionOutput::default();
                output.affinity = vis_system.affinity();
                return (*name, Ok(output));
            }

            re_tracing::profile_scope!("VisualizerSystem::execute", name.as_str());
            let affinity = vis_system.affinity();
            let result = vis_system
                .execute(&view_ctx, query, context_systems)
                .map(|mut output| {
                    output.affinity = affinity;
                    output
                });
            (*name, result)
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

    let mut active_visualizer_instructions_per_type = VisualizerInstructionsPerType::default();
    {
        re_tracing::profile_scope!("active_visualizer_instructions_per_type");

        for data_result in query_result.tree.iter_data_results() {
            if !data_result.visible {
                continue;
            }

            for instruction in &data_result.visualizer_instructions {
                active_visualizer_instructions_per_type
                    .entry(instruction.visualizer_type)
                    .or_default()
                    .push((data_result, instruction));
            }
        }
    }

    let current_query = ctx.time_ctrl.current_query();
    re_viewer_context::ViewQuery {
        view_id: view.id,
        space_origin: &view.space_origin,
        active_visualizer_instructions_per_type,
        timeline: *ctx.time_ctrl.timeline_name(),
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
    let view_systems = ctx
        .view_class_registry()
        .new_visualizer_collection(view.class_identifier());

    let visualizer_execution_output = run_view_systems(
        ctx,
        view,
        &query,
        view_state,
        context_system_once_per_frame_results,
        &mut context_systems,
        &view_systems,
    );

    (
        query,
        SystemExecutionOutput {
            context_systems,
            visualizer_execution_output,
        },
    )
}
