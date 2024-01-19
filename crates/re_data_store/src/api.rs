//! Higher-level query APIs.

use itertools::Itertools;
use re_log_types::{
    DataCell, DataReadError, DataReadResult, DataRow, DataTable, EntityPath, RowId, TimeInt,
    TimePoint,
};
use re_types_core::ComponentName;

use crate::{ArrayExt, DataStore, LatestAtQuery, RangeQuery};

// --- LatestAt ---

/// Queries a single component from its own point-of-view as well as its cluster key, and
/// returns a [`DataTable`].
///
/// As the cluster key is guaranteed to always be present, the returned table can be joined
/// with any number of other table returned by this function [`latest_component`] and
/// [`latest_components`].
///
/// See `example/latest_component.rs` for an example of use.
///
/// # Temporal semantics
///
/// Temporal indices take precedence, then timeless indices are queried to fill the holes left
/// by missing temporal data.
pub fn latest_component(
    store: &DataStore,
    query: &LatestAtQuery,
    entity_path: &EntityPath,
    primary: ComponentName,
) -> Option<DataRow> {
    let cluster_key = store.cluster_key();
    let components = &[cluster_key, primary];
    let (time, row_id, cells) = store.latest_at(query, entity_path, primary, components)?;
    DataRow::from_cells(
        row_id,
        time.map_or_else(TimePoint::timeless, |time| {
            TimePoint::from([(query.timeline, time)])
        }),
        entity_path.clone(),
        cells[0].as_ref().map_or(0, |cell| cell.num_instances()),
        cells.into_iter().flatten(),
    )
    .ok()
}
