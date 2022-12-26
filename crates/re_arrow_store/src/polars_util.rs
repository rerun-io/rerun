use arrow2::array::Array;
use itertools::Itertools;
use polars_core::{prelude::*, series::Series};
use re_log_types::{ComponentName, ObjPath as EntityPath, TimeInt};

use crate::{DataStore, LatestAtQuery, RangeQuery};

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
    let row_indices = store
        .latest_at(query, ent_path, primary, components)
        .unwrap_or([None; 2]);
    let results = store.get(components, &row_indices);

    dataframe_from_results(components, results)
}

/// Queries any number of components and their cluster keys from their respective point-of-views,
/// then joins all of them in one final `DataFrame` using the specified `join_type`.
///
/// As the cluster key is guaranteed to always be present, the returned dataframe can be joined
/// with any number of other dataframes returned by this function [`latest_component`] and
/// [`latest_components`].
///
/// See `example/latest_components.rs` for an example of use.
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
        .map(|primary| latest_component(store, query, ent_path, *primary));

    join_dataframes(cluster_key, join_type, dfs)
}

// --- Range ---

/// Iterates over the rows of a single component and its cluster key from the point-of-view of this
/// very same component, and returns an iterator of `DataFrame`s.
///
/// An initial dataframe is yielded with the latest-at state at the start of the time range, if
/// there is any.
///
/// ⚠ The semantics are subtle! See `example/range_component.rs` for an example of use.
pub fn range_component<'a>(
    store: &'a DataStore,
    query: &'a RangeQuery,
    ent_path: &'a EntityPath,
    primary: ComponentName,
) -> impl Iterator<Item = SharedResult<(TimeInt, DataFrame)>> + 'a {
    let cluster_key = store.cluster_key();

    let components = [cluster_key, primary];

    // Fetch the latest-at data just before the start of the time range.
    let latest_time = query.range.min.as_i64().checked_sub(1).map(Into::into);
    let df_latest = latest_time.map(|latest_time| {
        let query = LatestAtQuery::new(query.timeline, latest_time);
        let row_indices = store
            .latest_at(&query, ent_path, primary, &components)
            .unwrap_or([None; 2]);
        let results = store.get(&components, &row_indices);
        dataframe_from_results(&components, results).map(|df| (latest_time, df))
    });

    // Send the latest-at state before anything else..
    df_latest
        .into_iter()
        // ..but only if it's not an empty dataframe.
        .filter(|df| df.as_ref().map_or(true, |(_, df)| !df.is_empty()))
        .chain(store.range(query, ent_path, primary, components).map(
            move |(time, _, row_indices)| {
                let results = store.get(&components, &row_indices);
                dataframe_from_results(&components, results).map(|df| (time, df))
            },
        ))
}

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
pub fn range_components<'a>(
    store: &'a DataStore,
    query: &'a RangeQuery,
    ent_path: &'a EntityPath,
    primary: ComponentName,
    components: &[ComponentName],
    join_type: &'a JoinType,
) -> impl Iterator<Item = SharedResult<(TimeInt, DataFrame)>> + 'a {
    let cluster_key = store.cluster_key();

    let mut state: Vec<_> = std::iter::repeat_with(|| None)
        .take(components.len() + 1) // +1 for primary
        .collect();
    let mut iters: Vec<_> = std::iter::repeat_with(|| None)
        .take(components.len() + 1) // +1 for primary
        .collect();

    let latest_time = query.range.min.as_i64().checked_sub(1).map(Into::into);

    if let Some(latest_time) = latest_time {
        // Fetch the latest data for every single component from their respective point-of-views,
        // this will allow us to build up the initial state and send an initial latest-at
        // dataframe if needed.
        for (i, primary) in std::iter::once(&primary)
            .chain(components.iter())
            .enumerate()
        {
            let df = latest_component(
                store,
                &LatestAtQuery::new(query.timeline, latest_time),
                ent_path,
                *primary,
            );

            if df.as_ref().map_or(false, |df| !df.is_empty()) {
                state[i] = Some(df);
            }
        }
    }

    // Iff the primary component has a non-empty latest-at dataframe, then we want to be sending an
    // initial dataframe.
    let df_latest = if let (Some(latest_time), Some(_)) = (latest_time, &state[0]) {
        let df = join_dataframes(
            cluster_key,
            join_type,
            state.iter().filter_map(|df| df.as_ref()).cloned(), // shallow
        )
        .map(|df| (latest_time, df));
        Some(df)
    } else {
        None
    };

    // Now let's create the actual range iterators, one for each component / point-of-view.
    for (i, component) in std::iter::once(&primary)
        .chain(components.iter())
        .enumerate()
    {
        let components = [cluster_key, *component];

        let it = store.range(query, ent_path, *component, components).map(
            move |(time, idx_row_nr, row_indices)| {
                let results = store.get(&components, &row_indices);
                (
                    i,
                    time,
                    idx_row_nr,
                    dataframe_from_results(&components, results),
                )
            },
        );

        iters[i] = Some(it);
    }

    // Send the latest-at state before anything else..
    df_latest
        .into_iter()
        // ..but only if it's not an empty dataframe.
        .filter(|df| df.as_ref().map_or(true, |(_, df)| !df.is_empty()))
        .chain(
            iters
                .into_iter()
                .map(Option::unwrap)
                .kmerge_by(|(_, time1, idx_row_nr1, _), (_, time2, idx_row_nr2, _)| {
                    // Merge earlier rows first, and tiebreak on the actual bucket index row
                    // number if necessary!
                    (time1, idx_row_nr1) < (time2, idx_row_nr2)
                })
                .filter_map(move |(i, time, _, df)| {
                    state[i] = Some(df);

                    // We only yield if the primary component changes!
                    (i == 0).then(|| {
                        let df = join_dataframes(
                            cluster_key,
                            join_type,
                            state.iter().filter_map(|df| df.as_ref()).cloned(), // shallow
                        );
                        df.map(|df| (time, df))
                    })
                }),
        )
}

// --- Joins ---

pub fn dataframe_from_results<const N: usize>(
    components: &[ComponentName; N],
    results: [Option<Box<dyn Array>>; N],
) -> SharedResult<DataFrame> {
    let series: Result<Vec<_>, _> = components
        .iter()
        .zip(results)
        .filter_map(|(component, col)| col.map(|col| (component, col)))
        .map(|(&component, col)| Series::try_from((component.as_str(), col)))
        .collect();

    DataFrame::new(series?).map_err(Into::into)
}

pub fn join_dataframes(
    cluster_key: ComponentName,
    join_type: &JoinType,
    dfs: impl Iterator<Item = SharedResult<DataFrame>>,
) -> SharedResult<DataFrame> {
    let df = dfs
        .filter(|df| df.as_ref().map_or(true, |df| !df.is_empty()))
        .reduce(|acc, df| {
            acc?.join(
                &df?,
                [cluster_key.as_str()],
                [cluster_key.as_str()],
                join_type.clone(),
                None,
            )
            .map_err(Into::into)
        })
        .unwrap_or_else(|| Ok(DataFrame::default()))?;

    Ok(df.sort([cluster_key.as_str()], false).unwrap_or(df))
}
