//! Straightforward high-level API tests.
//!
//! Testing & demonstrating expected usage of the datastore APIs, no funny stuff.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

use arrow2::array::{Array, ListArray, UInt64Array};
use polars_core::{prelude::*, series::Series};
use re_arrow_store::{
    polars_util, test_bundle, DataStore, LatestAtQuery, RangeQuery, TimeInt, TimeRange,
};
use re_log_types::{
    datagen::{build_frame_nr, build_instances, build_some_point2d, build_some_rects},
    field_types::{Instance, Point2D, Rect2D},
    msg_bundle::{wrap_in_listarray, Component as _, MsgBundle},
    ComponentName, ObjPath as EntityPath, TimeType, Timeline,
};

// --- LatestAt ---

#[test]
fn latest_at() {
    init_logs();

    for config in re_arrow_store::test_util::all_configs() {
        let mut store = DataStore::new(Instance::name(), config.clone());
        latest_at_impl(&mut store);
    }
}
fn latest_at_impl(store: &mut DataStore) {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    let frame0 = 0.into();
    let frame1 = 1.into();
    let frame2 = 2.into();
    let frame3 = 3.into();
    let frame4 = 4.into();

    let (instances1, rects1) = (build_instances(3), build_some_rects(3));
    let bundle1 = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [instances1.clone(), rects1]);
    store.insert(&bundle1).unwrap();

    let points2 = build_some_point2d(3);
    let bundle2 = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [instances1, points2]);
    store.insert(&bundle2).unwrap();

    let points3 = build_some_point2d(10);
    let bundle3 = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [points3]);
    store.insert(&bundle3).unwrap();

    let rects4 = build_some_rects(5);
    let bundle4 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [rects4]);
    store.insert(&bundle4).unwrap();

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    let mut assert_latest_components =
        |frame_nr: TimeInt, bundles: &[(ComponentName, &MsgBundle)]| {
            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
            let components_all = &[Rect2D::name(), Point2D::name()];

            let df = polars_util::latest_components(
                store,
                &TimelineQuery::new(timeline_frame_nr, TimeQuery::LatestAt(frame_nr)),
                &ent_path,
                components_all,
            )
            .unwrap();

            let df_expected = joint_df(store.cluster_key(), bundles);

            store.sort_indices();
            assert_eq!(df_expected, df, "{store}");
        };

    // TODO(cmc): bring back some log_time scenarios

    assert_latest_components(frame0, &[]);
    assert_latest_components(frame1, &[(Rect2D::name(), &bundle1)]);
    assert_latest_components(
        frame2,
        &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle2)],
    );
    assert_latest_components(
        frame3,
        &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle3)],
    );
    assert_latest_components(
        frame4,
        &[(Rect2D::name(), &bundle4), (Point2D::name(), &bundle3)],
    );
}

// --- Range ---

#[test]
fn range() {
    init_logs();

    for config in re_arrow_store::test_util::all_configs() {
        let mut store = DataStore::new(Instance::name(), config.clone());
        range_impl(&mut store);
    }
}
fn range_impl(store: &mut DataStore) {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    let frame1 = 1.into();
    let frame2 = 2.into();
    let frame3 = 3.into();
    let frame4 = 4.into();
    let frame5 = 5.into();

    let (instances1, rects1) = (build_instances(3), build_some_rects(3));
    let bundle1 = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [instances1.clone(), rects1]);
    store.insert(&bundle1).unwrap();

    let points2 = build_some_point2d(3);
    let bundle2 = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [instances1, points2]);
    store.insert(&bundle2).unwrap();

    let points3 = build_some_point2d(10);
    let bundle3 = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [points3]);
    store.insert(&bundle3).unwrap();

    let rects4 = build_some_rects(5);
    let bundle4 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [rects4]);
    store.insert(&bundle4).unwrap();

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    // Unit-length time-ranges, should behave like a latest_at query at `start - 1`.

    assert_joint_range(
        store,
        &ent_path,
        TimeRange::new(frame1, frame1),
        Rect2D::name(),
        &[],
    );
    assert_joint_range(
        store,
        &ent_path,
        TimeRange::new(frame2, frame2),
        Rect2D::name(),
        &[(frame1, &[(Rect2D::name(), &bundle1)])],
    );
    assert_joint_range(
        store,
        &ent_path,
        TimeRange::new(frame3, frame3),
        Rect2D::name(),
        &[(
            frame2,
            &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle2)],
        )],
    );
    assert_joint_range(
        store,
        &ent_path,
        TimeRange::new(frame4, frame4),
        Rect2D::name(),
        &[(
            frame3,
            &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle3)],
        )],
    );
    assert_joint_range(
        store,
        &ent_path,
        TimeRange::new(frame5, frame5),
        Rect2D::name(),
        &[(
            frame4,
            &[(Rect2D::name(), &bundle4), (Point2D::name(), &bundle3)],
        )],
    );
}

/// Runs a joint query over all components at the given `frame_nr`, and asserts that the result
/// matches a joint `DataFrame` built ouf of the specified raw `bundles`.
fn assert_joint_range(
    store: &mut DataStore,
    ent_path: &EntityPath,
    time_range: TimeRange,
    primary: ComponentName,
    bundles_at_times: &[(TimeInt, &[(ComponentName, &MsgBundle)])],
) {
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let components_all = &[Instance::name(), Rect2D::name(), Point2D::name()];

    // let bundles_at_times: HashMap<TimeInt, &[(ComponentName, &MsgBundle)]> =
    //     bundles_at_times.iter().copied().collect();

    let query = RangeQuery::new(timeline_frame_nr, time_range);
    let dfs = range_query(store, &query, ent_path, primary, components_all);
    for (time, df) in dfs {
        eprintln!(
            "Found data at time {} from {}'s PoV (outer-joining):\n{:?}",
            query.timeline.typ().format(time),
            primary,
            df,
        );
    }
}

// --- Range helpers ---

// TODO: doc
fn range_query<'a, const N: usize>(
    store: &'a DataStore,
    query: &'a RangeQuery,
    ent_path: &'a EntityPath,
    primary: ComponentName,
    components: &'a [ComponentName; N],
) -> impl Iterator<Item = (TimeInt, DataFrame)> + 'a {
    store
        .range(query, ent_path, primary, components)
        .map(move |(time, row_indices)| {
            let df = {
                let results = store.get(components, &row_indices);

                let series: Vec<_> = components
                    .iter()
                    .zip(results)
                    .filter_map(|(component, col)| col.map(|col| (component, col)))
                    .map(|(&component, col)| Series::try_from((component.as_str(), col)).unwrap())
                    .collect();

                DataFrame::new(series).unwrap()
            };

            let df = std::iter::once(df)
                .reduce(|acc, df| {
                    acc.outer_join(
                        &df,
                        [Instance::name().as_str()],
                        [Instance::name().as_str()],
                    )
                    .unwrap()
                })
                .unwrap_or_default();

            let missing = components
                .iter()
                .enumerate()
                .filter_map(|(i, component)| row_indices[i].is_none().then_some(*component))
                .collect::<Vec<_>>();
            let df_missing = joint_latest_at_query(
                store,
                &LatestAtQuery::new(query.timeline, time),
                ent_path,
                &missing,
            );

            (time, join_dataframes([df, df_missing].into_iter()))
        })
}

fn join_dataframes(dfs: impl Iterator<Item = DataFrame>) -> DataFrame {
    let df = dfs
        .filter(|df| !df.is_empty())
        .reduce(|acc, df| {
            acc.outer_join(
                &df,
                [Instance::name().as_str()],
                [Instance::name().as_str()],
            )
            .unwrap()
        })
        .unwrap_or_default();

    df.sort([Instance::name().as_str()], false).unwrap_or(df)
}

// --- Common helpers ---

/// Given a list of bundles, crafts a `latest_components`-looking dataframe.
fn joint_df(cluster_key: ComponentName, bundles: &[(ComponentName, &MsgBundle)]) -> DataFrame {
    let df = bundles
        .iter()
        .map(|(component, bundle)| {
            let instances = if bundle.components.len() == 1 {
                let len = bundle.components[0]
                    .value
                    .as_any()
                    .downcast_ref::<ListArray<i32>>()
                    .unwrap()
                    .value(0)
                    .len();
                Series::try_from((
                    cluster_key.as_str(),
                    wrap_in_listarray(UInt64Array::from_vec((0..len as u64).collect()).to_boxed())
                        .to_boxed(),
                ))
                .unwrap()
            } else {
                Series::try_from((cluster_key.as_str(), bundle.components[0].value.to_boxed()))
                    .unwrap()
            };

            let df = DataFrame::new(vec![
                instances,
                Series::try_from((
                    component.as_str(),
                    bundle.components.last().unwrap().value.to_boxed(),
                ))
                .unwrap(),
            ])
            .unwrap();

            df.explode(df.get_column_names()).unwrap()
        })
        .reduce(|acc, df| {
            acc.outer_join(&df, [cluster_key.as_str()], [cluster_key.as_str()])
                .unwrap()
        })
        .unwrap_or_default();

    df.sort([Instance::name().as_str()], false).unwrap_or(df)
}

// ---

pub fn init_logs() {
    static INIT: AtomicBool = AtomicBool::new(false);

    if INIT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        re_log::set_default_rust_log_env();
        tracing_subscriber::fmt::init(); // log to stdout
    }
}
