use re_chunk_store::{LatestAtQuery, RangeQuery, RowId};
use re_log_types::{EntityPath, TimeInt};
use re_sdk_types::archetypes::Clear;
use re_viewer_context::ViewContext;

/// Collect the `(time, row_id)` of every `Clear` archetype that applies to `entity_path`
/// over `query`'s time range.
///
/// A `Clear` applies if it was logged on `entity_path` itself, or on any ancestor with
/// `is_recursive = true`. The bootstrap `latest_at` at `query.range.min()` also picks
/// up clears that landed just before the visible window.
///
/// Visualizers that need to render discontinuities (e.g. line plots breaking across a
/// reset, state lanes ending) feed these times back into their data.
pub fn collect_recursive_clears(
    ctx: &ViewContext<'_>,
    query: &RangeQuery,
    entity_path: &EntityPath,
) -> Vec<(TimeInt, RowId)> {
    re_tracing::profile_function!();

    let mut cleared_indices = Vec::new();

    let mut clear_entity_path = entity_path.clone();
    let clear_descriptor = Clear::descriptor_is_recursive();

    // Bootstrap: pick up any `Clear` in effect at the start of the visible range.
    {
        let results = ctx.recording_engine().cache().latest_at(
            &LatestAtQuery::new(query.timeline, query.range.min()),
            &clear_entity_path,
            [clear_descriptor.component],
        );

        cleared_indices.extend(
            results
                .get(clear_descriptor.component)
                .iter()
                .flat_map(|chunk| {
                    itertools::izip!(
                        chunk.iter_component_indices(*query.timeline(), clear_descriptor.component),
                        chunk.iter_slices::<bool>(clear_descriptor.component)
                    )
                })
                .filter_map(|(index, is_recursive_buffer)| {
                    let is_recursive =
                        !is_recursive_buffer.is_empty() && is_recursive_buffer.value(0);
                    (is_recursive || clear_entity_path == *entity_path).then_some(index)
                }),
        );
    }

    loop {
        let results = ctx.recording_engine().cache().range(
            query,
            &clear_entity_path,
            [clear_descriptor.component],
        );

        cleared_indices.extend(
            results
                .get(clear_descriptor.component)
                .unwrap_or_default()
                .iter()
                .flat_map(|chunk| {
                    itertools::izip!(
                        chunk.iter_component_indices(*query.timeline(), clear_descriptor.component),
                        chunk.iter_slices::<bool>(clear_descriptor.component)
                    )
                })
                .filter_map(|(index, is_recursive_buffer)| {
                    let is_recursive =
                        !is_recursive_buffer.is_empty() && is_recursive_buffer.value(0);
                    (is_recursive || clear_entity_path == *entity_path).then_some(index)
                }),
        );

        let Some(parent_entity_path) = clear_entity_path.parent() else {
            break;
        };

        clear_entity_path = parent_entity_path;
    }

    cleared_indices
}
