use re_data_store::{DataStore, LatestAtQuery};
use re_log_types::EntityPath;
use re_types_core::ComponentName;

use crate::LatestAtResults;

// ---

/// Queries for the given `component_names` using latest-at semantics.
///
/// See [`LatestAtResults`] for more information about how to handle the results.
///
/// This is a direct API -- no caching involved.
pub fn latest_at(
    store: &DataStore,
    query: &LatestAtQuery,
    entity_path: &EntityPath,
    component_names: impl IntoIterator<Item = ComponentName>,
) -> LatestAtResults {
    re_tracing::profile_function!(entity_path.to_string());

    let mut results = LatestAtResults::default();

    for component_name in component_names {
        let Some((time, row_id, mut cells)) =
            store.latest_at(query, entity_path, component_name, &[component_name])
        else {
            continue;
        };

        // Soundness:
        // * `cells[0]` is guaranteed to exist since we passed in `&[component_name]`
        // * `cells[0]` is guaranteed to be non-null, otherwise this whole result would be null
        if let Some(cell) = cells[0].take() {
            results.add(component_name, (time, row_id), cell);
        } else {
            debug_assert!(cells[0].is_some(), "unreachable: `cells[0]` is missing");
        }
    }

    results
}
