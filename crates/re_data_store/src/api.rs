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

/// Queries any number of components and their cluster keys from their respective point-of-views,
/// then joins all of them in one final [`DataTable`] using an outer join.
///
/// As the cluster key is guaranteed to always be present, the returned table can be joined
/// with any number of other tables returned by this function [`latest_component`] and
/// [`latest_components`].
///
/// See `example/latest_components.rs` for an example of use.
///
/// # Temporal semantics
///
/// Temporal indices take precedence, then timeless indices are queried to fill the holes left
/// by missing temporal data.
pub fn latest_components(
    store: &DataStore,
    query: &LatestAtQuery,
    ent_path: &EntityPath,
    primaries: &[ComponentName],
) -> Option<DataTable> {
    let cluster_key = store.cluster_key();

    let rows = primaries
        .iter()
        .filter(|primary| **primary != cluster_key)
        .filter_map(|primary| latest_component(store, query, ent_path, *primary));

    join_rows(cluster_key, rows)
}

/// Reduces an iterator of [`DataTable`]s into a single table by sequentially outer-joining them using
/// the specified `cluster_key`.
///
/// Note that if both the accumulator and the next table in the stream share a column name
/// (other than the cluster key), the column data from the next table takes precedence and
/// completely overwrites the current column data in the accumulator!
pub fn join_rows(
    cluster_key: ComponentName,
    rows: impl Iterator<Item = DataRow>,
) -> Option<DataTable> {
    let df = rows
        .into_iter()
        .filter(|row| row.cells().iter().flatten().next().is_some())
        .reduce(|left, right| {
            // If both `left` and `right` have data for the same column, `right` always takes
            // precedence.
            for col in right
                .get_column_names()
                .iter()
                .filter(|col| *col != &cluster_key)
            {
                _ = left.drop_in_place(col);
            }

            left.join(
                &right,
                [cluster_key],
                [cluster_key],
                join_type.clone(),
                None,
            )
            .map(|df| drop_all_nulls(&df, &cluster_key).unwrap())
            .map_err(Into::into)
        })
        .unwrap_or_else(|| Ok(DataFrame::default()))?;

    Ok(df.sort([cluster_key.as_str()], false).unwrap_or(df))
}
//
// /// Returns a new `DataFrame` where all rows that only contain null values (ignoring the cluster
// /// column) are dropped.
// pub fn drop_all_nulls(table: &DataTable, cluster_key: &ComponentName) -> DataTable {
//     let mut table = table.clone();
//     let cols = table
//         .get_column_names()
//         .into_iter()
//         .filter(|col| *col != cluster_key.as_str());
//
//     let mut iter = table.select_series(cols)?.into_iter();
//
//     // fast path for no nulls in df
//     if iter.clone().all(|s| !s.has_validity()) {
//         return Ok(table.clone());
//     }
//
//     let mask = iter
//         .next()
//         .ok_or_else(|| PolarsError::NoData("No data to drop nulls from".into()))?;
//     let mut mask = mask.is_not_null();
//
//     for s in iter {
//         mask = mask | s.is_not_null();
//     }
//     table.filter(&mask)
// }
