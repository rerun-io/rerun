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
    datagen::{
        build_frame_nr, build_some_instances, build_some_instances_from, build_some_point2d,
        build_some_rects,
    },
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

    let (instances1, rects1) = (build_some_instances(3), build_some_rects(3));
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
                &JoinType::Outer,
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

    let insts1 = build_some_instances(3);
    let rects1 = build_some_rects(3);
    let bundle1 = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [insts1.clone(), rects1]);
    store.insert(&bundle1).unwrap();

    let points2 = build_some_point2d(3);
    let bundle2 = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [insts1, points2]);
    store.insert(&bundle2).unwrap();

    let points3 = build_some_point2d(10);
    let bundle3 = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [points3]);
    store.insert(&bundle3).unwrap();

    let insts4_1 = build_some_instances_from(20..25);
    let rects4_1 = build_some_rects(5);
    let bundle4_1 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1, rects4_1]);
    store.insert(&bundle4_1).unwrap();

    let insts4_2 = build_some_instances_from(25..30);
    let rects4_2 = build_some_rects(5);
    let bundle4_2 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_2, rects4_2]);
    store.insert(&bundle4_2).unwrap();

    let insts4_3 = build_some_instances_from(30..35);
    let rects4_3 = build_some_rects(5);
    let bundle4_3 =
        test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_3.clone(), rects4_3]);
    store.insert(&bundle4_3).unwrap();

    let points4_4 = build_some_point2d(5);
    let bundle4_4 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_3, points4_4]);
    store.insert(&bundle4_4).unwrap();

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices();
        eprintln!("{store}");
        err.unwrap();
    }

    // Each entry in `bundles_at_times` corresponds to a dataframe that's expected to be returned
    // by the range query.
    // A single timepoint might have several of those! That's one of the behavior specific to range
    // queries.
    let mut assert_range_components =
        |time_range: TimeRange,
         components: [ComponentName; 2],
         bundles_at_times: &[(TimeInt, &[(ComponentName, &MsgBundle)])]| {
            let mut expected_at_times: IntMap<TimeInt, Vec<DataFrame>> = Default::default();

            for (time, bundles) in bundles_at_times {
                let dfs = expected_at_times.entry(*time).or_default();
                dfs.push(joint_df(store.cluster_key(), bundles));
            }

            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

            store.sort_indices(); // for assertions below

            let query = RangeQuery::new(timeline_frame_nr, time_range);
            let dfs = polars_util::range_components_4_real(
                store,
                &query,
                &ent_path,
                components,
                &JoinType::Outer,
            );

            let mut dfs_processed = 0usize;
            let mut time_counters: IntMap<i64, usize> = Default::default();
            for (time, df) in dfs.map(Result::unwrap) {
                let time_count = time_counters.entry(time.as_i64()).or_default();
                let df_expected = &expected_at_times[&time][*time_count];
                *time_count += 1;

                assert_eq!(*df_expected, df, "{store}");

                dfs_processed += 1;
            }

            let dfs_processed_expected = bundles_at_times.len();
            assert_eq!(dfs_processed_expected, dfs_processed);
        };

    // TODO(cmc): bring back some log_time scenarios

    // Unit ranges (Rect2D's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame1),
        [Rect2D::name(), Point2D::name()],
        &[
            (frame1, &[(Rect2D::name(), &bundle1)]), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame2, frame2),
        [Rect2D::name(), Point2D::name()],
        &[
            (frame1, &[(Rect2D::name(), &bundle1)]), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame3, frame3),
        [Rect2D::name(), Point2D::name()],
        &[
            (
                frame2,
                &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle2)],
            ), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame4, frame4),
        [Rect2D::name(), Point2D::name()],
        &[
            (
                frame3,
                &[(Rect2D::name(), &bundle1), (Point2D::name(), &bundle3)],
            ),
            // Do take note of the value of Point2D in all 3 cases below, which is effectively
            // "newer" than the primary component itself!
            // That's because `polars_util::range_components` takes some shortcuts rather than
            // implementing a true ordered streaming-join operator.
            (
                frame4,
                &[(Rect2D::name(), &bundle4_1), (Point2D::name(), &bundle4_4)], // !!!
            ),
            (
                frame4,
                &[(Rect2D::name(), &bundle4_2), (Point2D::name(), &bundle4_4)], // !!!
            ),
            (
                frame4,
                &[(Rect2D::name(), &bundle4_3), (Point2D::name(), &bundle4_4)], // !!!
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame5, frame5),
        [Rect2D::name(), Point2D::name()],
        &[
            (
                frame4,
                &[(Rect2D::name(), &bundle4_3), (Point2D::name(), &bundle4_4)],
            ), //
        ],
    );

    // Unit ranges (Point2D's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame1),
        [Point2D::name(), Rect2D::name()],
        &[],
    );
    assert_range_components(
        TimeRange::new(frame2, frame2),
        [Point2D::name(), Rect2D::name()],
        &[
            (
                frame2,
                &[(Point2D::name(), &bundle2), (Rect2D::name(), &bundle1)],
            ), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame3, frame3),
        [Point2D::name(), Rect2D::name()],
        &[
            (
                frame2,
                &[(Point2D::name(), &bundle2), (Rect2D::name(), &bundle1)],
            ),
            (
                frame3,
                &[(Point2D::name(), &bundle3), (Rect2D::name(), &bundle1)],
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame4, frame4),
        [Point2D::name(), Rect2D::name()],
        &[
            (
                frame3,
                &[(Point2D::name(), &bundle3), (Rect2D::name(), &bundle1)],
            ),
            (
                frame4,
                &[(Point2D::name(), &bundle4_4), (Rect2D::name(), &bundle4_3)],
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame5, frame5),
        [Point2D::name(), Rect2D::name()],
        &[
            (
                frame4,
                &[(Point2D::name(), &bundle4_4), (Rect2D::name(), &bundle4_3)],
            ), //
        ],
    );

    // Full range (Rect2D's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame5),
        [Rect2D::name(), Point2D::name()],
        &[
            (frame1, &[(Rect2D::name(), &bundle1)]),
            // Do take note of the value of Point2D in all 3 cases below, which is effectively
            // "newer" than the primary component itself!
            // That's because `polars_util::range_components` takes some shortcuts rather than
            // implementing a true ordered streaming-join operator.
            (
                frame4,
                &[(Rect2D::name(), &bundle4_1), (Point2D::name(), &bundle4_4)],
            ),
            (
                frame4,
                &[(Rect2D::name(), &bundle4_2), (Point2D::name(), &bundle4_4)],
            ),
            (
                frame4,
                &[(Rect2D::name(), &bundle4_3), (Point2D::name(), &bundle4_4)],
            ),
        ],
    );

    // Full range (Point2D's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame5),
        [Rect2D::name(), Point2D::name()],
        &[
            (
                frame2,
                &[(Point2D::name(), &bundle2), (Rect2D::name(), &bundle1)],
            ),
            (
                frame3,
                &[(Point2D::name(), &bundle3), (Rect2D::name(), &bundle1)],
            ),
            (
                frame4,
                &[(Point2D::name(), &bundle4_4), (Rect2D::name(), &bundle4_3)],
            ),
        ],
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
