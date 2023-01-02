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
/// Usage:
/// ```
/// # use re_arrow_store::{test_bundle, DataStore, LatestAtQuery, TimeType, Timeline};
/// # use re_arrow_store::polars_util::latest_component;
/// # use re_log_types::{
/// #     datagen::{build_frame_nr, build_some_point2d},
/// #     field_types::{Instance, Point2D},
/// #     msg_bundle::Component,
/// #     ObjPath as EntityPath,
/// # };
///
/// let mut store = DataStore::new(Instance::name(), Default::default());
///
/// let ent_path = EntityPath::from("my/entity");
///
/// let bundle3 = test_bundle!(ent_path @ [build_frame_nr(3.into())] => [build_some_point2d(2)]);
/// store.insert(&bundle3).unwrap();
///
/// let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
/// let df = latest_component(
///     &store,
///     &LatestAtQuery::new(timeline_frame_nr, 10.into()),
///     &ent_path,
///     Point2D::name(),
/// )
/// .unwrap();
///
/// println!("{df:?}");
/// ```
///
/// Outputs:
/// ```text
/// ┌────────────────┬─────────────────────┐
/// │ rerun.instance ┆ rerun.point2d       │
/// │ ---            ┆ ---                 │
/// │ u64            ┆ struct[2]           │
/// ╞════════════════╪═════════════════════╡
/// │ 0              ┆ {3.339503,6.287318} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 1              ┆ {2.813822,9.160795} │
/// └────────────────┴─────────────────────┘
/// ```
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
/// Usage:
/// ```
/// # use polars_core::prelude::*;
/// # use re_arrow_store::{test_bundle, DataStore, LatestAtQuery, TimeType, Timeline};
/// # use re_arrow_store::polars_util::latest_components;
/// # use re_log_types::{
/// #     datagen::{build_frame_nr, build_some_point2d, build_some_rects},
/// #     field_types::{Instance, Point2D, Rect2D},
/// #     msg_bundle::Component,
/// #     ObjPath as EntityPath,
/// # };
///
/// let mut store = DataStore::new(Instance::name(), Default::default());
///
/// let ent_path = EntityPath::from("my/entity");
///
/// let bundle = test_bundle!(ent_path @ [build_frame_nr(3.into())] => [build_some_point2d(2)]);
/// store.insert(&bundle).unwrap();
///
/// let bundle = test_bundle!(ent_path @ [build_frame_nr(5.into())] => [build_some_rects(4)]);
/// store.insert(&bundle).unwrap();
///
/// let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
/// let df = latest_components(
///     &store,
///     &LatestAtQuery::new(timeline_frame_nr, 10.into()),
///     &ent_path,
///     &[Point2D::name(), Rect2D::name()],
///     &JoinType::Outer,
/// )
/// .unwrap();
///
/// println!("{df:?}");
/// ```
///
/// Outputs:
/// ```text
/// ┌────────────────┬─────────────────────┬───────────────────┐
/// │ rerun.instance ┆ rerun.point2d       ┆ rerun.rect2d      │
/// │ ---            ┆ ---                 ┆ ---               │
/// │ u64            ┆ struct[2]           ┆ struct[4]         │
/// ╞════════════════╪═════════════════════╪═══════════════════╡
/// │ 0              ┆ {2.936338,1.308388} ┆ {0.0,0.0,0.0,0.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 1              ┆ {0.924683,7.757691} ┆ {1.0,1.0,0.0,0.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 2              ┆ {null,null}         ┆ {2.0,2.0,1.0,1.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 3              ┆ {null,null}         ┆ {3.0,3.0,1.0,1.0} │
/// └────────────────┴─────────────────────┴───────────────────┘
/// ```
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
/// ⚠ The semantics are subtle! Study carefully the example below.
///
/// Usage:
/// ```
/// use polars_core::prelude::JoinType;
/// use re_arrow_store::{polars_util, test_bundle, DataStore, RangeQuery, TimeRange};
/// use re_log_types::{
///     datagen::{build_frame_nr, build_some_point2d, build_some_rects},
///     field_types::{Instance, Point2D, Rect2D},
///     msg_bundle::Component as _,
///     ObjPath as EntityPath, TimeType, Timeline,
/// };
///
/// let mut store = DataStore::new(Instance::name(), Default::default());
///
/// let ent_path = EntityPath::from("this/that");
///
/// let frame1 = 1.into();
/// let frame2 = 2.into();
/// let frame3 = 3.into();
/// let frame4 = 4.into();
///
/// let bundle = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [build_some_rects(2)]);
/// store.insert(&bundle).unwrap();
///
/// let bundle = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [build_some_point2d(2)]);
/// store.insert(&bundle).unwrap();
///
/// let bundle = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [build_some_point2d(4)]);
/// store.insert(&bundle).unwrap();
///
/// let bundle = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [build_some_rects(3)]);
/// store.insert(&bundle).unwrap();
///
/// let bundle = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [build_some_point2d(1)]);
/// store.insert(&bundle).unwrap();
///
/// let bundle = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [build_some_rects(3)]);
/// store.insert(&bundle).unwrap();
///
/// let bundle = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [build_some_point2d(3)]);
/// store.insert(&bundle).unwrap();
///
/// let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
/// let query = RangeQuery {
///     timeline: timeline_frame_nr,
///     range: TimeRange::new(2.into(), 4.into()),
/// };
///
/// let dfs = polars_util::range_components(
///     &store,
///     &query,
///     &ent_path,
///     Rect2D::name(),
///     [Instance::name(), Rect2D::name(), Point2D::name()],
///     &JoinType::Outer,
/// );
///
/// for (time, df) in dfs.map(Result::unwrap) {
///     eprintln!(
///         "Found data at time {} from {}'s PoV (outer-joining):\n{:?}",
///         TimeType::Sequence.format(time),
///         Rect2D::name(),
///         df,
///     );
/// }
/// ```
///
/// Outputs:
/// ```text
/// Found data at time #1 from rerun.rect2d's PoV (outer-joining):
/// ┌────────────────┬───────────────────┐
/// │ rerun.instance ┆ rerun.rect2d      │
/// │ ---            ┆ ---               │
/// │ u64            ┆ struct[4]         │
/// ╞════════════════╪═══════════════════╡
/// │ 0              ┆ {0.0,0.0,0.0,0.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 1              ┆ {1.0,1.0,0.0,0.0} │
/// └────────────────┴───────────────────┘
///
/// Found data at time #4 from rerun.rect2d's PoV (outer-joining):
/// ┌────────────────┬───────────────────────┬───────────────┐
/// │ rerun.instance ┆ rerun.rect2d          ┆ rerun.point2d │
/// │ ---            ┆ ---                   ┆ ---           │
/// │ u64            ┆ struct[4]             ┆ struct[2]     │
/// ╞════════════════╪═══════════════════════╪═══════════════╡
/// │ 0              ┆ {0.0,0.0,0.0,0.0}     ┆ {20.0,20.0}   │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 1              ┆ {1.0,1.0,0.0,0.0}     ┆ {21.0,21.0}   │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 2              ┆ {2.0,2.0,1.0,1.0}     ┆ {22.0,22.0}   │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 3              ┆ {null,null,null,null} ┆ {23.0,23.0}   │
/// └────────────────┴───────────────────────┴───────────────┘
///
/// Found data at time #4 from rerun.rect2d's PoV (outer-joining):
/// ┌────────────────┬───────────────────┬───────────────┐
/// │ rerun.instance ┆ rerun.rect2d      ┆ rerun.point2d │
/// │ ---            ┆ ---               ┆ ---           │
/// │ u64            ┆ struct[4]         ┆ struct[2]     │
/// ╞════════════════╪═══════════════════╪═══════════════╡
/// │ 0              ┆ {0.0,0.0,0.0,0.0} ┆ {30.0,30.0}   │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 1              ┆ {1.0,1.0,0.0,0.0} ┆ {null,null}   │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 2              ┆ {2.0,2.0,1.0,1.0} ┆ {null,null}   │
/// └────────────────┴───────────────────┴───────────────┘
/// ``
pub fn range_components<'a, const N: usize>(
    store: &'a DataStore,
    query: &'a RangeQuery,
    ent_path: &'a EntityPath,
    primary: ComponentName,
    components: [ComponentName; N],
    join_type: &'a JoinType,
) -> impl Iterator<Item = SharedResult<(TimeInt, DataFrame)>> + 'a {
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

    let latest_time = query.range.min.as_i64().checked_sub(1).map(Into::into);

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
            state = Some(df);
        }
    }

    let df_latest = if let (Some(latest_time), Some(state)) = (latest_time, state.as_ref()) {
        Some(state.clone().map(|df| (latest_time, df)) /* shallow */)
    } else {
        None
    };

    let primary_col = components
        .iter()
        .find_position(|component| **component == primary)
        .map(|(col, _)| col)
        .unwrap(); // asserted on entry

    let range = store
        .range(query, ent_path, components)
        .map(move |(time, _, row_indices)| {
            let results = store.get(&components, &row_indices);
            (
                time,
                row_indices[primary_col].is_some(), // is_primary
                dataframe_from_results(&components, results),
            )
        });

    // Send the latest-at state before anything else
    df_latest
        .into_iter()
        .chain(range.filter_map(move |(time, is_primary, df)| {
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
        }))
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
                left = left.drop_nulls(None).unwrap();
            }

            left.join(
                &right,
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
