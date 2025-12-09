use re_log_types::{TimeInt, TimelineName};
use re_types::Archetype;
use re_view::{AnnotationSceneContext, ChunksWithComponent, DataResultQuery as _, HybridResults};
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError,
};

use crate::contexts::{EntityDepthOffsets, SpatialSceneEntityContext, TransformTreeContext};

// --- Chunk-based APIs ---

/// Iterates through all entity views for a given archetype.
///
/// The callback passed in gets passed along a [`SpatialSceneEntityContext`] which contains
/// various useful information about an entity in the context of the current scene.
pub fn process_archetype<System: IdentifiedViewSystem, A, F>(
    ctx: &ViewContext<'_>,
    query: &ViewQuery<'_>,
    context_systems: &ViewContextCollection,
    mut fun: F,
) -> Result<(), ViewSystemExecutionError>
where
    A: Archetype,
    F: FnMut(
        &QueryContext<'_>,
        &SpatialSceneEntityContext<'_>,
        &HybridResults<'_>,
    ) -> Result<(), ViewSystemExecutionError>,
{
    let transforms = context_systems.get::<TransformTreeContext>()?;
    let depth_offsets = context_systems.get::<EntityDepthOffsets>()?;
    let annotations = context_systems.get::<AnnotationSceneContext>()?;

    let latest_at = query.latest_at_query();

    let system_identifier = System::identifier();

    for data_result in query.iter_visible_data_results(system_identifier) {
        let Some(transform_info) =
            transforms.transform_info_for_entity(data_result.entity_path.hash())
        else {
            continue;
        };

        let depth_offset_key = (system_identifier, data_result.entity_path.hash());
        let entity_context = SpatialSceneEntityContext {
            transform_info,
            depth_offset: depth_offsets
                .per_entity_and_visualizer
                .get(&depth_offset_key)
                .copied()
                .unwrap_or_default(),
            annotations: annotations.0.find(&data_result.entity_path),
            highlight: query
                .highlights
                .entity_outline_mask(data_result.entity_path.hash()),
            view_class_identifier: context_systems.view_class_identifier(),
        };

        let results = data_result.query_archetype_with_history::<A>(ctx, query);

        let mut query_ctx = ctx.query_context(data_result, &latest_at);
        query_ctx.archetype_name = Some(A::name());

        {
            re_tracing::profile_scope!(format!("{}", data_result.entity_path));
            fun(&query_ctx, &entity_context, &results)?;
        }
    }

    Ok(())
}

// ---

use re_chunk::{ChunkComponentIterItem, RowId};
use re_chunk_store::external::re_chunk;

/// Iterate `chunks` as indexed deserialized batches.
///
/// For simple cases (i.e. everything up to flat structs), prefer [`iter_slices`] instead which is
/// faster.
///
/// See [`re_chunk::Chunk::iter_component`] for more information.
pub fn iter_component<'a, C: re_types::Component>(
    chunks: &'a ChunksWithComponent<'a>,
    timeline: TimelineName,
) -> impl Iterator<Item = ((TimeInt, RowId), ChunkComponentIterItem<C>)> + 'a {
    chunks.iter().flat_map(move |chunk| {
        itertools::izip!(
            chunk.iter_component_indices(timeline),
            chunk.iter_component::<C>()
        )
    })
}

/// Iterate `chunks` as indexed primitives.
///
/// See [`re_chunk::Chunk::iter_slices`] for more information.
pub fn iter_slices<'a, T: 'a + re_chunk::ChunkComponentSlicer>(
    chunks: &'a ChunksWithComponent<'a>,
    timeline: TimelineName,
) -> impl Iterator<Item = ((TimeInt, RowId), T::Item<'a>)> + 'a {
    chunks.iter().flat_map(move |chunk| {
        itertools::izip!(
            chunk.iter_component_indices(timeline),
            chunk.iter_slices::<T>()
        )
    })
}
