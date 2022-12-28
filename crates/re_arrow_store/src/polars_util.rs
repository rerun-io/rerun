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
/// ⚠ The semantics are subtle! Study carefully the example below.
///
/// Usage:
/// ```
/// # use re_arrow_store::{polars_util, test_bundle, DataStore, RangeQuery, TimeRange};
/// # use re_log_types::{
/// #     datagen::{
/// #         build_frame_nr, build_some_instances, build_some_instances_from, build_some_point2d,
/// #         build_some_rects,
/// #     },
/// #     field_types::{Instance, Rect2D},
/// #     msg_bundle::Component as _,
/// #     ObjPath as EntityPath, TimeType, Timeline,
/// # };
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
/// let insts1 = build_some_instances(2);
/// let rects1 = build_some_rects(2);
/// let bundle1 = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [insts1.clone(), rects1]);
/// store.insert(&bundle1).unwrap();
///
/// let points2 = build_some_point2d(2);
/// let bundle2 = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [insts1, points2]);
/// store.insert(&bundle2).unwrap();
///
/// let insts3 = build_some_instances_from(25..29);
/// let points3 = build_some_point2d(4);
/// let bundle3 = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [insts3, points3]);
/// store.insert(&bundle3).unwrap();
///
/// let insts4_1 = build_some_instances_from(20..23);
/// let rects4_1 = build_some_rects(3);
/// let bundle4_1 =
///     test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1.clone(), rects4_1]);
/// store.insert(&bundle4_1).unwrap();
///
/// let points4_15 = build_some_point2d(3);
/// let bundle4_15 =
///     test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1.clone(), points4_15]);
/// store.insert(&bundle4_15).unwrap();
///
/// let insts4_2 = build_some_instances_from(25..28);
/// let rects4_2 = build_some_rects(3);
/// let bundle4_2 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_2, rects4_2]);
/// store.insert(&bundle4_2).unwrap();
///
/// let points4_25 = build_some_point2d(3);
/// let bundle4_25 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1, points4_25]);
/// store.insert(&bundle4_25).unwrap();
///
/// let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
/// let query = RangeQuery {
///     timeline: timeline_frame_nr,
///     range: TimeRange::new(2.into(), 4.into()),
/// };
///
/// let dfs = polars_util::range_component(&store, &query, &ent_path, Rect2D::name());
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
/// │ 1              ┆ {0.0,0.0,0.0,0.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 10             ┆ {1.0,1.0,0.0,0.0} │
/// └────────────────┴───────────────────┘
///
/// Found data at time #4 from rerun.rect2d's PoV (outer-joining):
/// ┌────────────────┬───────────────────┐
/// │ rerun.instance ┆ rerun.rect2d      │
/// │ ---            ┆ ---               │
/// │ u64            ┆ struct[4]         │
/// ╞════════════════╪═══════════════════╡
/// │ 20             ┆ {0.0,0.0,0.0,0.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 21             ┆ {1.0,1.0,0.0,0.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 22             ┆ {2.0,2.0,1.0,1.0} │
/// └────────────────┴───────────────────┘
///
/// Found data at time #4 from rerun.rect2d's PoV (outer-joining):
/// ┌────────────────┬───────────────────┐
/// │ rerun.instance ┆ rerun.rect2d      │
/// │ ---            ┆ ---               │
/// │ u64            ┆ struct[4]         │
/// ╞════════════════╪═══════════════════╡
/// │ 25             ┆ {0.0,0.0,0.0,0.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 26             ┆ {1.0,1.0,0.0,0.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 27             ┆ {2.0,2.0,1.0,1.0} │
/// └────────────────┴───────────────────┘
/// ```
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
/// ⚠ The semantics are subtle! Study carefully the example below.
///
/// Usage:
/// ```
/// # use polars_core::prelude::JoinType;
/// # use re_arrow_store::{polars_util, test_bundle, DataStore, RangeQuery, TimeRange};
/// # use re_log_types::{
/// #     datagen::{
/// #         build_frame_nr, build_some_instances, build_some_instances_from, build_some_point2d,
/// #         build_some_rects,
/// #     },
/// #     field_types::{Instance, Rect2D, Point2D},
/// #     msg_bundle::Component as _,
/// #     ObjPath as EntityPath, TimeType, Timeline,
/// # };
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
/// let insts1 = build_some_instances(2);
/// let rects1 = build_some_rects(2);
/// let bundle1 = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [insts1.clone(), rects1]);
/// store.insert(&bundle1).unwrap();
///
/// let points2 = build_some_point2d(2);
/// let bundle2 = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [insts1, points2]);
/// store.insert(&bundle2).unwrap();
///
/// let insts3 = build_some_instances_from(25..29);
/// let points3 = build_some_point2d(4);
/// let bundle3 = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [insts3, points3]);
/// store.insert(&bundle3).unwrap();
///
/// let insts4_1 = build_some_instances_from(20..23);
/// let rects4_1 = build_some_rects(3);
/// let bundle4_1 =
///     test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1.clone(), rects4_1]);
/// store.insert(&bundle4_1).unwrap();
///
/// let points4_15 = build_some_point2d(3);
/// let bundle4_15 =
///     test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1.clone(), points4_15]);
/// store.insert(&bundle4_15).unwrap();
///
/// let insts4_2 = build_some_instances_from(25..28);
/// let rects4_2 = build_some_rects(3);
/// let bundle4_2 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_2, rects4_2]);
/// store.insert(&bundle4_2).unwrap();
///
/// let points4_25 = build_some_point2d(3);
/// let bundle4_25 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1, points4_25]);
/// store.insert(&bundle4_25).unwrap();
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
///     &[Point2D::name()],
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
/// │ 7              ┆ {0.0,0.0,0.0,0.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 19             ┆ {1.0,1.0,0.0,0.0} │
/// └────────────────┴───────────────────┘
///
/// Found data at time #4 from rerun.rect2d's PoV (outer-joining):
/// ┌────────────────┬───────────────────────┬─────────────────────┐
/// │ rerun.instance ┆ rerun.rect2d          ┆ rerun.point2d       │
/// │ ---            ┆ ---                   ┆ ---                 │
/// │ u64            ┆ struct[4]             ┆ struct[2]           │
/// ╞════════════════╪═══════════════════════╪═════════════════════╡
/// │ 20             ┆ {0.0,0.0,0.0,0.0}     ┆ {null,null}         │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 21             ┆ {1.0,1.0,0.0,0.0}     ┆ {null,null}         │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 22             ┆ {2.0,2.0,1.0,1.0}     ┆ {null,null}         │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 25             ┆ {null,null,null,null} ┆ {4.674534,1.10232}  │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 26             ┆ {null,null,null,null} ┆ {5.485249,3.561962} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 27             ┆ {null,null,null,null} ┆ {1.286991,7.455362} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 28             ┆ {null,null,null,null} ┆ {5.445724,9.622441} │
/// └────────────────┴───────────────────────┴─────────────────────┘
///
/// Found data at time #4 from rerun.rect2d's PoV (outer-joining):
/// ┌────────────────┬───────────────────────┬─────────────────────┐
/// │ rerun.instance ┆ rerun.rect2d          ┆ rerun.point2d       │
/// │ ---            ┆ ---                   ┆ ---                 │
/// │ u64            ┆ struct[4]             ┆ struct[2]           │
/// ╞════════════════╪═══════════════════════╪═════════════════════╡
/// │ 20             ┆ {null,null,null,null} ┆ {2.220385,9.471127} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 21             ┆ {null,null,null,null} ┆ {2.006991,0.522795} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 22             ┆ {null,null,null,null} ┆ {4.77748,0.148467}  │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 25             ┆ {0.0,0.0,0.0,0.0}     ┆ {null,null}         │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 26             ┆ {1.0,1.0,0.0,0.0}     ┆ {null,null}         │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 27             ┆ {2.0,2.0,1.0,1.0}     ┆ {null,null}         │
/// └────────────────┴───────────────────────┴─────────────────────┘
/// ```
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
                .kmerge_by(|(i1, time1, idx_row_nr1, _), (i2, time2, idx_row_nr2, _)| {
                    // # Understanding the merge order
                    //
                    // We first compare the timestamps, of course: the lower of the two gets merged
                    // first.
                    // If the timestamps are equal, then we use the opaque `IndexBucketRowNr` that
                    // the datastore gives us in order to tiebreak the two.
                    //
                    // We're not over, though: it can happen that the index row numbers are
                    // themselves equal! This means that for this specific entry, the two iterators
                    // actually share the exact same row in the datastore.
                    // In that case, we always want the primary/point-of-view iterator to come
                    // last, so that it can gather as much state as possible before yielding!
                    //
                    // Read closely: `i2` is on the left of the < operator!
                    (time1, idx_row_nr1, i2) < (time2, idx_row_nr2, i1)
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
