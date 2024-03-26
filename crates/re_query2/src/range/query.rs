use re_data_store::{DataStore, RangeQuery};
use re_log_types::EntityPath;
use re_types_core::ComponentName;

use crate::RangeResults;

// ---

/// Queries for the given `component_names` using range semantics.
///
/// See [`RangeResults`] for more information about how to handle the results.
pub fn range(
    store: &DataStore,
    query: &RangeQuery,
    entity_path: &EntityPath,
    component_names: impl IntoIterator<Item = ComponentName>,
) -> RangeResults {
    re_tracing::profile_function!(entity_path.to_string());

    let mut results = RangeResults::default();

    for component_name in component_names {
        let data = store.range(query, entity_path, [component_name]).map(
            |(data_time, row_id, mut cells)| {
                // Unwrap:
                // * `cells[0]` is guaranteed to exist since we passed in `&[component_name]`
                // * `cells[0]` is guaranteed to be non-null, otherwise this whole result would be null
                let cell = cells[0].take().unwrap();

                ((data_time, row_id), cell)
            },
        );

        results.add(component_name, data);
    }

    results
}
