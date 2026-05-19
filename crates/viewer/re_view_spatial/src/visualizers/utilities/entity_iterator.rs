use re_sdk_types::Archetype;
use re_view::{AnnotationSceneContext, DataResultQuery as _, VisualizerInstructionQueryResults};
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerSystem,
};

use crate::contexts::{
    EntityDepthOffsets, SpatialSceneVisualizerInstructionContext, TransformTreeContext,
};
use crate::visualizers::utilities::{
    spatial_view_kind_from_affinity, transform_info_for_archetype_or_report_error,
};

// --- Chunk-based APIs ---

/// Iterates through all entity views for a given archetype.
///
/// The callback passed in gets passed along a [`SpatialSceneVisualizerInstructionContext`] which contains
/// various useful information about an entity in the context of the current scene.
///
/// The visualizer's [`VisualizerSystem::affinity`] is used to determine its preferred spatial view kind (2D or 3D).
/// Based on this, 2D entities require a pinhole parent in 3D views, and 3D entities require a pinhole at the root of 2D views.
pub fn process_archetype<A, F, V: VisualizerSystem + IdentifiedViewSystem>(
    ctx: &ViewContext<'_>,
    query: &ViewQuery<'_>,
    context_systems: &ViewContextCollection,
    output: &VisualizerExecutionOutput,
    visualizer: &V,
    mut fun: F,
) -> Result<(), ViewSystemExecutionError>
where
    A: Archetype,
    F: FnMut(
        &QueryContext<'_>,
        &SpatialSceneVisualizerInstructionContext<'_>,
        &VisualizerInstructionQueryResults<'_>,
    ) -> Result<(), ViewSystemExecutionError>,
{
    re_tracing::profile_function!(V::identifier());

    let view_kind = super::spatial_view_kind_from_view_class(ctx.view_class_identifier);
    let transforms = context_systems.get::<TransformTreeContext>(output)?;
    let depth_offsets = context_systems.get::<EntityDepthOffsets>(output)?;
    let annotations = context_systems.get::<AnnotationSceneContext>(output)?;

    let latest_at = query.latest_at_query();

    let system_identifier = V::identifier();
    let archetype_kind = spatial_view_kind_from_affinity(visualizer.affinity());

    for (data_result, visualizer_instruction) in
        query.iter_visualizer_instruction_for(system_identifier)
    {
        let entity_path = &data_result.entity_path;

        let Some(transform_info) = transform_info_for_archetype_or_report_error(
            entity_path,
            transforms,
            archetype_kind,
            view_kind,
            &visualizer_instruction.id,
            output,
        ) else {
            continue;
        };

        let depth_offset_key = (system_identifier, entity_path.hash());
        let instruction_context = SpatialSceneVisualizerInstructionContext {
            visualizer_instruction: visualizer_instruction.id,
            transform_info,
            depth_offset: depth_offsets
                .per_entity_and_visualizer
                .get(&depth_offset_key)
                .copied()
                .unwrap_or_default(),
            annotations: annotations.0.find(entity_path),
            highlight: query.highlights.entity_outline_mask(entity_path.hash()),
            view_class_identifier: context_systems.view_class_identifier(),
        };

        let results =
            data_result.query_archetype_with_history::<A>(ctx, query, visualizer_instruction);

        let visualizer_instruction_result =
            VisualizerInstructionQueryResults::new(visualizer_instruction, &results, output);

        let mut query_ctx =
            ctx.query_context(data_result, latest_at.clone(), visualizer_instruction.id);
        query_ctx.archetype_name = Some(A::name());

        {
            re_tracing::profile_scope!(format!("{entity_path}"));
            fun(
                &query_ctx,
                &instruction_context,
                &visualizer_instruction_result,
            )?;
        }
    }

    Ok(())
}
