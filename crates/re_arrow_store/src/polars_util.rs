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

    let series: Result<Vec<_>, _> = components
        .iter()
        .zip(results)
        .filter_map(|(component, col)| col.map(|col| (component, col)))
        .map(|(&component, col)| Series::try_from((component.as_str(), col)))
        .collect();

    DataFrame::new(series?).map_err(Into::into)
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
        .map(|primary| latest_component(store, query, ent_path, *primary))
        .filter(|df| df.as_ref().map(|df| !df.is_empty()).unwrap_or(true));

    let df = dfs
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

// --- Range ---

// TODO: doc
pub fn range_component<'a>(
    store: &'a DataStore,
    query: &'a RangeQuery,
    ent_path: &'a EntityPath,
    primary: ComponentName,
) -> impl Iterator<Item = anyhow::Result<(TimeInt, DataFrame)>> + 'a {
    let cluster_key = store.cluster_key();

    let components = [cluster_key, primary];
    store
        .range(query, ent_path, primary, components)
        .map(move |(time, row_indices)| {
            let results = store.get(&components, &row_indices);
            let series: Result<Vec<_>, _> = components
                .iter()
                .zip(results)
                .filter_map(|(component, col)| col.map(|col| (component, col)))
                .map(|(&component, col)| Series::try_from((component.as_str(), col)))
                .collect();

            Ok::<_, anyhow::Error>((time, DataFrame::new(series?)?))
        })
}

// TODO: doc
pub fn range_components<'a, const N: usize>(
    store: &'a DataStore,
    query: &'a RangeQuery,
    ent_path: &'a EntityPath,
    primary: ComponentName,
    components: [ComponentName; N],
    join_type: &'a JoinType,
) -> impl Iterator<Item = anyhow::Result<(TimeInt, DataFrame)>> + 'a {
    let cluster_key = store.cluster_key();

    assert!(
        components.contains(&cluster_key),
        "`components` must contain the cluster key, got {components:?}, \
            which is missing {cluster_key:?}",
    );

    store
        .range(query, ent_path, primary, components)
        .map(move |(time, row_indices)| {
            let df = {
                let results = store.get(&components, &row_indices);
                let series: Result<Vec<_>, _> = components
                    .iter()
                    .zip(results)
                    .filter_map(|(component, col)| col.map(|col| (component, col)))
                    .map(|(&component, col)| Series::try_from((component.as_str(), col)))
                    .collect();
                DataFrame::new(series?)?
            };

            let missing = components
                .iter()
                .enumerate()
                .filter_map(|(i, component)| row_indices[i].is_none().then_some(*component))
                .collect::<Vec<_>>();
            let df_missing = latest_components(
                store,
                &LatestAtQuery::new(query.timeline, time),
                ent_path,
                &missing,
                join_type,
            )
            .unwrap();

            Ok((
                time,
                join_dataframes(cluster_key, join_type, [df, df_missing].into_iter())?,
            ))
        })
}

// --- Joins ---

pub fn join_dataframes(
    cluster_key: ComponentName,
    join_type: &JoinType,
    dfs: impl Iterator<Item = DataFrame>,
) -> anyhow::Result<DataFrame> {
    let df = dfs
        .filter(|df| !df.is_empty())
        .map(Ok::<_, anyhow::Error>)
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
