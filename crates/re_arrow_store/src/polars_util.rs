use itertools::Itertools;
use polars_core::{prelude::*, series::Series};
use polars_ops::prelude::*;
use re_log_types::{ComponentName, DataCell, EntityPath, RowId, TimeInt};

use crate::{ArrayExt, DataStore, LatestAtQuery, RangeQuery};

// ---

pub type SharedPolarsError = Arc<PolarsError>;

pub type SharedResult<T> = ::std::result::Result<T, SharedPolarsError>;

// --- LatestAt ---

/// Queries a single component from its own point-of-view as well as its cluster key, and
/// returns a `DataFrame`.
///
/// As the cluster key is guaranteed to always be present, the returned dataframe can be joined
/// with any number of other dataframes returned by this function [`latest_component`] and
/// [`latest_components`].
///
/// See `example/latest_component.rs` for an example of use.
///
/// # Temporal semantics
///
/// Temporal indices take precedence, then timeless indices are queried to fill the holes left
/// by missing temporal data.
//
// TODO(cmc): can this really fail though?
pub fn latest_component(
    store: &DataStore,
    query: &LatestAtQuery,
    ent_path: &EntityPath,
    primary: ComponentName,
) -> SharedResult<DataFrame> {
    let cluster_key = store.cluster_key();

    let components = &[cluster_key, primary];
    let (_, cells) = store
        .latest_at(query, ent_path, primary, components)
        .unwrap_or((RowId::ZERO, [(); 2].map(|_| None)));

    dataframe_from_cells(&cells)
}

/// Queries any number of components and their cluster keys from their respective point-of-views,
/// then joins all of them in one final `DataFrame` using the specified `join_type`.
///
/// As the cluster key is guaranteed to always be present, the returned dataframe can be joined
/// with any number of other dataframes returned by this function [`latest_component`] and
/// [`latest_components`].
///
/// See `example/latest_components.rs` for an example of use.
///
/// # Temporal semantics
///
/// Temporal indices take precedence, then timeless indices are queried to fill the holes left
/// by missing temporal data.
//
// TODO(cmc): can this really fail though?
pub fn latest_components(
    store: &DataStore,
    query: &LatestAtQuery,
    ent_path: &EntityPath,
    primaries: &[ComponentName],
    join_type: &JoinType,
) -> SharedResult<DataFrame> {
    let cluster_key = store.cluster_key();

    let dfs = primaries
        .iter()
        .filter(|primary| **primary != cluster_key)
        .map(|primary| latest_component(store, query, ent_path, *primary));

    join_dataframes(cluster_key, join_type, dfs)
}

// --- Range ---

/// Iterates over the rows of any number of components and their respective cluster keys, all from
/// the single point-of-view of the `primary` component, returning an iterator of `DataFrame`s.
///
/// An initial dataframe is yielded with the latest-at state at the start of the time range, if
/// there is any.
///
/// The iterator only ever yields dataframes iff the `primary` component has changed.
/// A change affecting only secondary components will not yield a dataframe.
///
/// This is a streaming-join: every yielded dataframe will be the result of joining the latest
/// known state of all components, from their respective point-of-views.
///
/// ⚠ The semantics are subtle! See `example/range_components.rs` for an example of use.
///
/// # Temporal semantics
///
/// Yields the contents of the temporal indices.
/// Iff the query's time range starts at `TimeInt::MIN`, this will yield the contents of the
/// timeless indices before anything else.
///
/// When yielding timeless entries, the associated time will be `None`.
pub fn range_components<'a, const N: usize>(
    store: &'a DataStore,
    query: &'a RangeQuery,
    ent_path: &'a EntityPath,
    primary: ComponentName,
    components: [ComponentName; N],
    join_type: &'a JoinType,
) -> impl Iterator<Item = SharedResult<(Option<TimeInt>, DataFrame)>> + 'a {
    let cluster_key = store.cluster_key();

    // TODO(cmc): Ideally, we'd want to simply add the cluster and primary key to the `components`
    // array if they are missing, yielding either `[ComponentName; N+1]` or `[ComponentName; N+2]`.
    // Unfortunately this is not supported on stable at the moment, and requires
    // feature(generic_const_exprs) on nightly.
    //
    // The alternative to these assertions (and thus putting the burden on the caller), for now,
    // would be to drop the constant sizes all the way down, which would be way more painful to
    // deal with.
    assert!(components.contains(&cluster_key));
    assert!(components.contains(&primary));

    let mut state = None;

    // NOTE: This will return none for `TimeInt::Min`, i.e. range queries that start infinitely far
    // into the past don't have a latest-at state!
    let latest_time = query.range.min.as_i64().checked_sub(1).map(Into::into);

    let mut df_latest = None;
    if let Some(latest_time) = latest_time {
        let df = latest_components(
            store,
            &LatestAtQuery::new(query.timeline, latest_time),
            ent_path,
            &components,
            join_type,
        );

        if df.as_ref().map_or(false, |df| {
            // We only care about the initial state if it A) isn't empty and B) contains any data
            // at all for the primary component.
            !df.is_empty() && df.column(primary.as_str()).is_ok()
        }) {
            df_latest = Some(df);
        }
    }

    let primary_col = components
        .iter()
        .find_position(|component| **component == primary)
        .map(|(col, _)| col)
        .unwrap(); // asserted on entry

    // send the latest-at state before anything else
    df_latest
        .into_iter()
        .map(move |df| (latest_time, true, df))
        // followed by the range
        .chain(
            store
                .range(query, ent_path, components)
                .map(move |(time, _, cells)| {
                    (
                        time,
                        cells[primary_col].is_some(), // is_primary
                        dataframe_from_cells(&cells),
                    )
                }),
        )
        .filter_map(move |(time, is_primary, df)| {
            state = Some(join_dataframes(
                cluster_key,
                join_type,
                // The order matters here: the newly yielded dataframe goes to the right so that it
                // overwrites the data in the state if their column overlaps!
                // See [`join_dataframes`].
                [state.clone() /* shallow */, Some(df)]
                    .into_iter()
                    .flatten(),
            ));

            // We only yield if the primary component has been updated!
            is_primary.then_some(state.clone().unwrap().map(|df| {
                // Make sure to return everything in the order it was asked!
                let columns = df.get_column_names();
                let df = df
                    .select(
                        components
                            .iter()
                            .filter(|col| columns.contains(&col.as_str())),
                    )
                    .unwrap();
                (time, df)
            }))
        })
}

// --- Joins ---

// TODO(#1759): none of this mess should be here

pub fn dataframe_from_cells<const N: usize>(
    cells: &[Option<DataCell>; N],
) -> SharedResult<DataFrame> {
    let series: Result<Vec<_>, _> = cells
        .iter()
        .flatten()
        .map(|cell| {
            Series::try_from((
                cell.component_name().as_str(),
                cell.as_arrow_ref().clean_for_polars(),
            ))
        })
        .collect();

    DataFrame::new(series?).map_err(Into::into)
}

/// Reduces an iterator of dataframes into a single dataframe by sequentially joining them using
/// the specified `join_type` and `cluster_key`.
///
/// Note that if both the accumulator and the next dataframe in the stream share a column name
/// (other than the cluster key), the column data from the next dataframe takes precedence and
/// completely overwrites the current column data in the accumulator!
pub fn join_dataframes(
    cluster_key: ComponentName,
    join_type: &JoinType,
    dfs: impl Iterator<Item = SharedResult<DataFrame>>,
) -> SharedResult<DataFrame> {
    let df = dfs
        .into_iter()
        .filter(|df| df.as_ref().map_or(true, |df| !df.is_empty()))
        .reduce(|left, right| {
            let mut left = left?;
            let right = right?;

            // If both `left` and `right` have data for the same column, `right` always takes
            // precedence.
            for col in right
                .get_column_names()
                .iter()
                .filter(|col| *col != &cluster_key.as_str())
            {
                _ = left.drop_in_place(col);
            }

            left.join(
                &right,
                [cluster_key.as_str()],
                [cluster_key.as_str()],
                join_type.clone(),
                None,
            )
            .map(|df| drop_all_nulls(&df, &cluster_key).unwrap())
            .map_err(Into::into)
        })
        .unwrap_or_else(|| Ok(DataFrame::default()))?;

    Ok(df.sort([cluster_key.as_str()], false).unwrap_or(df))
}

/// Returns a new `DataFrame` where all rows that only contain null values (ignoring the cluster
/// column) are dropped.
pub fn drop_all_nulls(df: &DataFrame, cluster_key: &ComponentName) -> PolarsResult<DataFrame> {
    let cols = df
        .get_column_names()
        .into_iter()
        .filter(|col| *col != cluster_key.as_str());

    let mut iter = df.select_series(cols)?.into_iter();

    // fast path for no nulls in df
    if iter.clone().all(|s| !s.has_validity()) {
        return Ok(df.clone());
    }

    let mask = iter
        .next()
        .ok_or_else(|| PolarsError::NoData("No data to drop nulls from".into()))?;
    let mut mask = mask.is_not_null();

    for s in iter {
        mask = mask | s.is_not_null();
    }
    df.filter(&mask)
}
