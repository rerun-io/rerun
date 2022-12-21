//! Straightforward high-level API tests.
//!
//! Testing & demonstrating expected usage of the datastore APIs, no funny stuff.

use std::sync::atomic::{AtomicBool, Ordering};

use arrow2::array::{Array, ListArray, UInt64Array};
use nohash_hasher::IntMap;
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
                &LatestAtQuery::new(timeline_frame_nr, frame_nr),
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

    let mut assert_range_components =
        |time_range: TimeRange,
         primary: ComponentName,
         bundles_at_times: &[(TimeInt, &[(ComponentName, &MsgBundle)])]| {
            let bundles_at_times: IntMap<TimeInt, &[(ComponentName, &MsgBundle)]> =
                bundles_at_times.iter().copied().collect();

            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
            let components_all = &[store.cluster_key(), Rect2D::name(), Point2D::name()];

            let query = RangeQuery::new(timeline_frame_nr, time_range);
            let dfs =
                polars_util::range_components(store, &query, &ent_path, primary, components_all);

            for (time, df) in dfs.map(Result::unwrap) {
                let df_expected = joint_df(store.cluster_key(), bundles_at_times[&time]);

                eprintln!(
                    "Found data at time {} from {}'s PoV (outer-joining):\n{:?}",
                    TimeType::Sequence.format(time), // TODO
                    primary,
                    df,
                );

                // store.sort_indices();
                // assert_eq!(df_expected, df, "{store}");
            }
        };

    // TODO(cmc): bring back some log_time scenarios

    // Unit-length time-ranges, should behave like a latest_at query at `start - 1`.

    assert_range_components(TimeRange::new(frame1, frame1), Rect2D::name(), &[]);
    assert_range_components(
        TimeRange::new(frame2, frame2),
        Rect2D::name(),
        &[(frame1, &[(Rect2D::name(), &bundle1)])],
    );
    assert_range_components(
        TimeRange::new(frame3, frame3),
        Rect2D::name(),
        &[(
            frame2,
            &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle2)],
        )],
    );
    assert_range_components(
        TimeRange::new(frame4, frame4),
        Rect2D::name(),
        &[(
            frame3,
            &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle3)],
        )],
    );
    assert_range_components(
        TimeRange::new(frame5, frame5),
        Rect2D::name(),
        &[(
            frame4,
            &[(Rect2D::name(), &bundle4), (Point2D::name(), &bundle3)],
        )],
    );
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

    df.sort([cluster_key.as_str()], false).unwrap_or(df)
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
