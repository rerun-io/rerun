use arrow2::array::Array;
use itertools::Itertools;
use polars_core::{prelude::*, series::Series};
use re_log_types::{ComponentName, ObjPath as EntityPath, TimeInt};

use crate::{DataStore, LatestAtQuery, RangeQuery};

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
) -> anyhow::Result<DataFrame> {
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
) -> anyhow::Result<DataFrame> {
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
/// let bundle4_1 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1, rects4_1]);
/// store.insert(&bundle4_1).unwrap();
///
/// let insts4_2 = build_some_instances_from(25..28);
/// let rects4_2 = build_some_rects(3);
/// let bundle4_2 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_2, rects4_2]);
/// store.insert(&bundle4_2).unwrap();
///
/// let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
/// let query = RangeQuery {
///     timeline: timeline_frame_nr,
///     range: TimeRange::new(1.into(), 4.into()),
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
/// │ 16             ┆ {0.0,0.0,0.0,0.0} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 17             ┆ {1.0,1.0,0.0,0.0} │
/// └────────────────┴───────────────────┘
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
// TODO
pub fn range_component<'a>(
    store: &'a DataStore,
    query: &'a RangeQuery,
    ent_path: &'a EntityPath,
    primary: ComponentName,
) -> impl Iterator<Item = anyhow::Result<(TimeInt, DataFrame)>> + 'a {
    let cluster_key = store.cluster_key();

    let components = [cluster_key, primary];

    let latest_time = query.range.min.as_i64().saturating_sub(1).into();
    let df_latest = {
        let query = LatestAtQuery::new(query.timeline, latest_time);
        let row_indices = store
            .latest_at(&query, ent_path, primary, &components)
            .unwrap_or([None; 2]);
        let results = store.get(&components, &row_indices);
        dataframe_from_results(&components, results)
    };

    std::iter::once(df_latest.map(|df| (latest_time, df)))
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
/// For each dataframe yielded by this iterator, a latest-at query will be ran for all missing
/// secondary `components`, and the results joined together using the specified `join_type`.
/// Not that this can results in different behaviors compared to a "true" ordered streaming-join
/// operator!
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
/// let bundle4_1 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1, rects4_1]);
/// store.insert(&bundle4_1).unwrap();
///
/// let insts4_2 = build_some_instances_from(25..28);
/// let rects4_2 = build_some_rects(3);
/// let bundle4_2 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_2, rects4_2]);
/// store.insert(&bundle4_2).unwrap();
///
/// let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
/// let query = RangeQuery {
///     timeline: timeline_frame_nr,
///     range: TimeRange::new(1.into(), 4.into()),
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
/// │ 5              ┆ {1.0,1.0,0.0,0.0} │
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
/// │ 25             ┆ {null,null,null,null} ┆ {6.365356,6.691178} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 26             ┆ {null,null,null,null} ┆ {6.310458,1.014078} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 27             ┆ {null,null,null,null} ┆ {5.565524,5.133609} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 28             ┆ {null,null,null,null} ┆ {4.919256,4.289873} │
/// └────────────────┴───────────────────────┴─────────────────────┘
///
/// Found data at time #4 from rerun.rect2d's PoV (outer-joining):
/// ┌────────────────┬───────────────────────┬─────────────────────┐
/// │ rerun.instance ┆ rerun.rect2d          ┆ rerun.point2d       │
/// │ ---            ┆ ---                   ┆ ---                 │
/// │ u64            ┆ struct[4]             ┆ struct[2]           │
/// ╞════════════════╪═══════════════════════╪═════════════════════╡
/// │ 25             ┆ {0.0,0.0,0.0,0.0}     ┆ {6.365356,6.691178} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 26             ┆ {1.0,1.0,0.0,0.0}     ┆ {6.310458,1.014078} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 27             ┆ {2.0,2.0,1.0,1.0}     ┆ {5.565524,5.133609} │
/// ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
/// │ 28             ┆ {null,null,null,null} ┆ {4.919256,4.289873} │
/// └────────────────┴───────────────────────┴─────────────────────┘
/// ```
// TODO
pub fn range_components<'a, const N: usize>(
    store: &'a DataStore,
    query: &'a RangeQuery,
    ent_path: &'a EntityPath,
    components: [ComponentName; N], // 1st is primary
    join_type: &'a JoinType,
) -> impl Iterator<Item = anyhow::Result<(TimeInt, DataFrame)>> + 'a {
    let cluster_key = store.cluster_key();

    let mut state = [(); N].map(|_| None);

    // TODO: explain why this is
    let latest_time = query.range.min.as_i64().saturating_sub(1).into();
    for (i, primary) in components.iter().enumerate() {
        let components = &[cluster_key, *primary];

        let query = LatestAtQuery::new(query.timeline, latest_time);
        let row_indices = store
            .latest_at(&query, ent_path, *primary, components)
            .unwrap_or([None; 2]);
        let results = store.get(components, &row_indices);

        let df = dataframe_from_results(components, results);

        if df.as_ref().map_or(false, |df| !df.is_empty()) {
            state[i] = Some(df);
        }
    }

    // TODO: explain why this is
    let df_latest = if state[0].is_some() {
        join_dataframes(
            cluster_key,
            join_type,
            state
                .iter()
                .filter_map(|df| df.as_ref())
                .map(|df| Ok(df.as_ref().unwrap().clone())), // TODO
        )
    } else {
        Ok(DataFrame::default())
    };

    // TODO: explain why this is
    let mut iters = [(); N].map(|_| None);
    for (i, component) in components.iter().enumerate() {
        let components = [cluster_key, *component];

        let it = store.range(query, ent_path, *component, components).map(
            move |(time, index_nr, row_indices)| {
                let results = store.get(&components, &row_indices);
                (
                    i,
                    time,
                    index_nr,
                    dataframe_from_results(&components, results),
                )
            },
        );

        iters[i] = Some(it);
    }

    std::iter::once(df_latest.map(|df| (latest_time, df)))
        .filter(|df| df.as_ref().map_or(true, |(_, df)| !df.is_empty()))
        .chain(
            iters
                .into_iter()
                .map(Option::unwrap) // TODO: explain
                .kmerge_by(|(_, time1, index_nr1, _), (_, time2, index_nr2, _)| {
                    (time1, index_nr1) < (time2, index_nr2) // TODO: explain
                })
                .filter_map(move |(i, time, _, df)| {
                    state[i] = Some(df);

                    if i == 0 {
                        let df = join_dataframes(
                            cluster_key,
                            join_type,
                            state
                                .iter()
                                .filter_map(|df| df.as_ref())
                                .map(|df| Ok(df.as_ref().unwrap().clone())), // TODO
                        );
                        Some(df.map(|df| (time, df)))
                    } else {
                        None
                    }
                }),
        )
}

// --- Joins ---

pub fn dataframe_from_results<const N: usize>(
    components: &[ComponentName; N],
    results: [Option<Box<dyn Array>>; N],
) -> anyhow::Result<DataFrame> {
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
    dfs: impl Iterator<Item = anyhow::Result<DataFrame>>,
) -> anyhow::Result<DataFrame> {
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
