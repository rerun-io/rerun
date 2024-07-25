use std::collections::BTreeSet;

use re_chunk_store::{ChunkStore, LatestAtQuery};
use re_entity_db::InstancePath;
use re_log_types::{EntityPath, Instance, Timeline};
use re_viewer_context::{ViewQuery, ViewerContext};

/// Returns a sorted list of all entities that are visible in the view.
pub(crate) fn sorted_visible_entity_path(
    ctx: &ViewerContext<'_>,
    query: &ViewQuery<'_>,
) -> BTreeSet<EntityPath> {
    query
        .iter_all_data_results()
        .filter(|data_result| data_result.is_visible(ctx))
        .map(|data_result| &data_result.entity_path)
        .cloned()
        .collect()
}

/// Returns a sorted, deduplicated iterator of all instance paths for a given entity.
pub(crate) fn sorted_instance_paths_for<'a>(
    entity_path: &'a EntityPath,
    store: &'a ChunkStore,
    timeline: &'a Timeline,
    latest_at_query: &'a LatestAtQuery,
) -> impl Iterator<Item = InstancePath> + 'a {
    re_tracing::profile_function!();

    store
        .all_components_on_timeline(timeline, entity_path)
        .unwrap_or_default()
        .into_iter()
        .filter(|component_name| !component_name.is_indicator_component())
        .flat_map(|component_name| {
            let num_instances = store
                .latest_at_relevant_chunks(latest_at_query, entity_path, component_name)
                .into_iter()
                .filter_map(|chunk| {
                    let (data_time, row_id, batch) = chunk
                        .latest_at(latest_at_query, component_name)
                        .iter_rows(timeline, &component_name)
                        .next()?;
                    batch.map(|batch| (data_time, row_id, batch))
                })
                .max_by_key(|(data_time, row_id, _)| (*data_time, *row_id))
                .map_or(0, |(_, _, batch)| batch.len());
            (0..num_instances).map(|i| Instance::from(i as u64))
        })
        .collect::<BTreeSet<_>>() // dedup and sort
        .into_iter()
        .map(|instance| InstancePath::instance(entity_path.clone(), instance))
}
