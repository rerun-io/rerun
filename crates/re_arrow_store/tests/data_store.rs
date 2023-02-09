//! Straightforward high-level API tests.
//!
//! Testing & demonstrating expected usage of the datastore APIs, no funny stuff.

use std::sync::atomic::{AtomicBool, Ordering};

use arrow2::array::{Array, UInt64Array};
use nohash_hasher::IntMap;
use polars_core::{prelude::*, series::Series};
use polars_ops::prelude::DataFrameJoinOps;
use rand::Rng;
use re_arrow_store::{
    polars_util, test_bundle, DataStore, DataStoreConfig, GarbageCollectionTarget, LatestAtQuery,
    RangeQuery, TimeInt, TimeRange,
};
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, Point2D, Rect2D},
    datagen::{
        build_frame_nr, build_some_colors, build_some_instances, build_some_instances_from,
        build_some_point2d, build_some_rects,
    },
    external::arrow2_convert::deserialize::arrow_array_deserialize_iterator,
    msg_bundle::{wrap_in_listarray, Component as _, MsgBundle},
    ComponentName, EntityPath, MsgId, TimeType, Timeline,
};

// --- LatestComponentsAt ---

#[test]
fn all_components() {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    // let frame0: TimeInt = 0.into();
    let frame1: TimeInt = 1.into();
    let frame2: TimeInt = 2.into();
    let frame3: TimeInt = 3.into();
    let frame4: TimeInt = 4.into();

    let assert_latest_components_at =
        |store: &mut DataStore, ent_path: &EntityPath, expected: Option<&[ComponentName]>| {
            let timeline = Timeline::new("frame_nr", TimeType::Sequence);

            let components = store.all_components(&timeline, ent_path);

            let components = components.map(|mut components| {
                components.sort();
                components
            });

            let expected = expected.map(|expected| {
                let mut expected = expected.to_vec();
                expected.sort();
                expected
            });

            store.sort_indices_if_needed();
            assert_eq!(
                expected, components,
                "expected to find {expected:?}, found {components:?} instead\n{store}",
            );
        };

    // One big bucket, demonstrating the easier-to-reason-about cases.
    {
        let mut store = DataStore::new(
            InstanceKey::name(),
            DataStoreConfig {
                component_bucket_nb_rows: u64::MAX,
                index_bucket_nb_rows: u64::MAX,
                ..Default::default()
            },
        );
        let cluster_key = store.cluster_key();

        let components_a = &[
            ColorRGBA::name(), // added by us, timeless
            Rect2D::name(),    // added by us
            cluster_key,       // always here
            MsgId::name(),     // automatically appended by MsgBundle
            #[cfg(debug_assertions)]
            DataStore::insert_id_key(), // automatically added in debug
        ];

        let components_b = &[
            ColorRGBA::name(), // added by us, timeless
            Point2D::name(),   // added by us
            Rect2D::name(),    // added by us
            cluster_key,       // always here
            MsgId::name(),     // automatically appended by MsgBundle
            #[cfg(debug_assertions)]
            DataStore::insert_id_key(), // automatically added in debug
        ];

        let bundle = test_bundle!(ent_path @ [] => [build_some_colors(2)]);
        store.insert(&bundle).unwrap();

        let bundle = test_bundle!(ent_path @ [
            build_frame_nr(frame1),
        ] => [build_some_rects(2)]);
        store.insert(&bundle).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let bundle = test_bundle!(ent_path @ [
            build_frame_nr(frame2),
        ] => [build_some_rects(2), build_some_point2d(2)]);
        store.insert(&bundle).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_b));

        if let err @ Err(_) = store.sanity_check() {
            store.sort_indices_if_needed();
            eprintln!("{store}");
            err.unwrap();
        }
    }

    // Tiny buckets, demonstrating the harder-to-reason-about cases.
    {
        let mut store = DataStore::new(
            InstanceKey::name(),
            DataStoreConfig {
                component_bucket_nb_rows: 0,
                index_bucket_nb_rows: 0,
                ..Default::default()
            },
        );
        let cluster_key = store.cluster_key();

        // ┌──────────┬────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ rect2d ┆ msg_id ┆ insert_id ┆ instance │
        // ╞══════════╪════════╪════════╪═══════════╪══════════╡
        // │ 1        ┆ 1      ┆ 1      ┆ 1         ┆ 1        │
        // └──────────┴────────┴────────┴───────────┴──────────┘
        // ┌──────────┬────────┬─────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ rect2d ┆ point2d ┆ msg_id ┆ insert_id ┆ instance │
        // ╞══════════╪════════╪═════════╪════════╪═══════════╪══════════╡
        // │ 2        ┆ -      ┆ -       ┆ 2      ┆ 2         ┆ 2        │
        // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
        // │ 3        ┆ -      ┆ 1       ┆ 3      ┆ 3         ┆ 1        │
        // └──────────┴────────┴─────────┴────────┴───────────┴──────────┘

        let components_a = &[
            ColorRGBA::name(), // added by us, timeless
            Rect2D::name(),    // added by us
            cluster_key,       // always here
            MsgId::name(),     // automatically appended by MsgBundle
            #[cfg(debug_assertions)]
            DataStore::insert_id_key(), // automatically added in debug
        ];

        let components_b = &[
            ColorRGBA::name(), // added by us, timeless
            Rect2D::name(),    // ⚠ inherited before the buckets got splitted apart!
            Point2D::name(),   // added by us
            cluster_key,       // always here
            MsgId::name(),     // automatically appended by MsgBundle
            #[cfg(debug_assertions)]
            DataStore::insert_id_key(), // automatically added in debug
        ];

        let bundle = test_bundle!(ent_path @ [] => [build_some_colors(2)]);
        store.insert(&bundle).unwrap();

        let bundle = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [build_some_rects(2)]);
        store.insert(&bundle).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let bundle = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [build_some_instances(2)]);
        store.insert(&bundle).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let bundle = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [build_some_point2d(2)]);
        store.insert(&bundle).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_b));

        if let err @ Err(_) = store.sanity_check() {
            store.sort_indices_if_needed();
            eprintln!("{store}");
            err.unwrap();
        }
    }

    // Tiny buckets and tricky splits, demonstrating a case that is not only extremely hard to
    // reason about, it is technically incorrect.
    {
        let mut store = DataStore::new(
            InstanceKey::name(),
            DataStoreConfig {
                component_bucket_nb_rows: 0,
                index_bucket_nb_rows: 0,
                ..Default::default()
            },
        );
        let cluster_key = store.cluster_key();

        // ┌──────────┬────────┬─────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ rect2d ┆ point2d ┆ msg_id ┆ insert_id ┆ instance │
        // ╞══════════╪════════╪═════════╪════════╪═══════════╪══════════╡
        // │ 1        ┆ -      ┆ 1       ┆ 4      ┆ 4         ┆ 1        │
        // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
        // │ 2        ┆ 1      ┆ -       ┆ 1      ┆ 1         ┆ 1        │
        // └──────────┴────────┴─────────┴────────┴───────────┴──────────┘
        // ┌──────────┬────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ rect2d ┆ msg_id ┆ insert_id ┆ instance │
        // ╞══════════╪════════╪════════╪═══════════╪══════════╡
        // │ 3        ┆ 2      ┆ 2      ┆ 2         ┆ 1        │
        // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
        // │ 4        ┆ 3      ┆ 3      ┆ 3         ┆ 1        │
        // └──────────┴────────┴────────┴───────────┴──────────┘

        let components_a = &[
            ColorRGBA::name(), // added by us, timeless
            Rect2D::name(),    // added by us
            cluster_key,       // always here
            MsgId::name(),     // automatically appended by MsgBundle
            #[cfg(debug_assertions)]
            DataStore::insert_id_key(), // automatically added in debug
        ];

        let components_b = &[
            ColorRGBA::name(), // added by us, timeless
            Point2D::name(),   // added by us but not contained in the second bucket
            Rect2D::name(),    // added by use
            cluster_key,       // always here
            MsgId::name(),     // automatically appended by MsgBundle
            #[cfg(debug_assertions)]
            DataStore::insert_id_key(), // automatically added in debug
        ];

        let bundle = test_bundle!(ent_path @ [] => [build_some_colors(2)]);
        store.insert(&bundle).unwrap();

        let bundle = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [build_some_rects(2)]);
        store.insert(&bundle).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let bundle = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [build_some_rects(2)]);
        store.insert(&bundle).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let bundle = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [build_some_rects(2)]);
        store.insert(&bundle).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let bundle = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [build_some_point2d(2)]);
        store.insert(&bundle).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_b));

        if let err @ Err(_) = store.sanity_check() {
            store.sort_indices_if_needed();
            eprintln!("{store}");
            err.unwrap();
        }
    }
}

// --- LatestAt ---

#[test]
fn latest_at() {
    init_logs();

    for config in re_arrow_store::test_util::all_configs() {
        let mut store = DataStore::new(InstanceKey::name(), config.clone());
        latest_at_impl(&mut store);
        store.gc(
            GarbageCollectionTarget::DropAtLeastPercentage(1.0),
            Timeline::new("frame_nr", TimeType::Sequence),
            MsgId::name(),
        );
        latest_at_impl(&mut store);
    }
}
fn latest_at_impl(store: &mut DataStore) {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    let frame0: TimeInt = 0.into();
    let frame1: TimeInt = 1.into();
    let frame2: TimeInt = 2.into();
    let frame3: TimeInt = 3.into();
    let frame4: TimeInt = 4.into();

    // helper to insert a bundle both as a temporal and timeless payload
    let insert = |store: &mut DataStore, bundle| {
        // insert temporal
        store.insert(bundle).unwrap();

        // insert timeless
        let mut bundle_timeless = bundle.clone();
        bundle_timeless.time_point = Default::default();
        store.insert(&bundle_timeless).unwrap();
    };

    let (instances1, colors1) = (build_some_instances(3), build_some_colors(3));
    let bundle1 =
        test_bundle!(ent_path @ [build_frame_nr(frame1)] => [instances1.clone(), colors1]);
    insert(store, &bundle1);

    let points2 = build_some_point2d(3);
    let bundle2 = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [instances1, points2]);
    insert(store, &bundle2);

    let points3 = build_some_point2d(10);
    let bundle3 = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [points3]);
    insert(store, &bundle3);

    let colors4 = build_some_colors(5);
    let bundle4 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [colors4]);
    insert(store, &bundle4);

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices_if_needed();
        eprintln!("{store}");
        err.unwrap();
    }

    let mut assert_latest_components =
        |frame_nr: TimeInt, bundles: &[(ComponentName, &MsgBundle)]| {
            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
            let components_all = &[ColorRGBA::name(), Point2D::name()];

            let df = polars_util::latest_components(
                store,
                &LatestAtQuery::new(timeline_frame_nr, frame_nr),
                &ent_path,
                components_all,
                &JoinType::Outer,
            )
            .unwrap();

            let df_expected = joint_df(store.cluster_key(), bundles);

            store.sort_indices_if_needed();
            assert_eq!(df_expected, df, "{store}");
        };

    // TODO(cmc): bring back some log_time scenarios

    assert_latest_components(
        frame0,
        &[(ColorRGBA::name(), &bundle4), (Point2D::name(), &bundle3)], // timeless
    );
    assert_latest_components(
        frame1,
        &[
            (ColorRGBA::name(), &bundle1),
            (Point2D::name(), &bundle3), // timeless
        ],
    );
    assert_latest_components(
        frame2,
        &[(ColorRGBA::name(), &bundle1), (Point2D::name(), &bundle2)],
    );
    assert_latest_components(
        frame3,
        &[(ColorRGBA::name(), &bundle1), (Point2D::name(), &bundle3)],
    );
    assert_latest_components(
        frame4,
        &[(ColorRGBA::name(), &bundle4), (Point2D::name(), &bundle3)],
    );
}

// --- Range ---

#[test]
fn range() {
    init_logs();

    for config in re_arrow_store::test_util::all_configs() {
        let mut store = DataStore::new(InstanceKey::name(), config.clone());
        range_impl(&mut store);
    }
}
fn range_impl(store: &mut DataStore) {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    let frame0: TimeInt = 0.into();
    let frame1: TimeInt = 1.into();
    let frame2: TimeInt = 2.into();
    let frame3: TimeInt = 3.into();
    let frame4: TimeInt = 4.into();
    let frame5: TimeInt = 5.into();

    // helper to insert a bundle both as a temporal and timeless payload
    let insert = |store: &mut DataStore, bundle| {
        // insert temporal
        store.insert(bundle).unwrap();

        // insert timeless
        let mut bundle_timeless = bundle.clone();
        bundle_timeless.time_point = Default::default();
        store.insert(&bundle_timeless).unwrap();
    };

    let insts1 = build_some_instances(3);
    let colors1 = build_some_colors(3);
    let bundle1 = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [insts1.clone(), colors1]);
    insert(store, &bundle1);

    let points2 = build_some_point2d(3);
    let bundle2 = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [insts1, points2]);
    insert(store, &bundle2);

    let points3 = build_some_point2d(10);
    let bundle3 = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [points3]);
    insert(store, &bundle3);

    let insts4_1 = build_some_instances_from(20..25);
    let colors4_1 = build_some_colors(5);
    let bundle4_1 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1, colors4_1]);
    insert(store, &bundle4_1);

    let insts4_2 = build_some_instances_from(25..30);
    let colors4_2 = build_some_colors(5);
    let bundle4_2 =
        test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_2.clone(), colors4_2]);
    insert(store, &bundle4_2);

    let points4_25 = build_some_point2d(5);
    let bundle4_25 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_2, points4_25]);
    insert(store, &bundle4_25);

    let insts4_3 = build_some_instances_from(30..35);
    let colors4_3 = build_some_colors(5);
    let bundle4_3 =
        test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_3.clone(), colors4_3]);
    insert(store, &bundle4_3);

    let points4_4 = build_some_point2d(5);
    let bundle4_4 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_3, points4_4]);
    insert(store, &bundle4_4);

    if let err @ Err(_) = store.sanity_check() {
        store.sort_indices_if_needed();
        eprintln!("{store}");
        err.unwrap();
    }

    // Each entry in `bundles_at_times` corresponds to a dataframe that's expected to be returned
    // by the range query.
    // A single timepoint might have several of those! That's one of the behaviors specific to
    // range queries.
    #[allow(clippy::type_complexity)]
    let mut assert_range_components =
        |time_range: TimeRange,
         components: [ComponentName; 2],
         bundles_at_times: &[(Option<TimeInt>, &[(ComponentName, &MsgBundle)])]| {
            let mut expected_timeless = Vec::<DataFrame>::new();
            let mut expected_at_times: IntMap<TimeInt, Vec<DataFrame>> = Default::default();

            for (time, bundles) in bundles_at_times {
                if let Some(time) = time {
                    let dfs = expected_at_times.entry(*time).or_default();
                    dfs.push(joint_df(store.cluster_key(), bundles));
                } else {
                    expected_timeless.push(joint_df(store.cluster_key(), bundles));
                }
            }

            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

            store.sort_indices_if_needed(); // for assertions below

            let components = [InstanceKey::name(), components[0], components[1]];
            let query = RangeQuery::new(timeline_frame_nr, time_range);
            let dfs = polars_util::range_components(
                store,
                &query,
                &ent_path,
                components[1],
                components,
                &JoinType::Outer,
            );

            let mut dfs_processed = 0usize;
            let mut timeless_count = 0usize;
            let mut time_counters: IntMap<i64, usize> = Default::default();
            for (time, df) in dfs.map(Result::unwrap) {
                let df_expected = if let Some(time) = time {
                    let time_count = time_counters.entry(time.as_i64()).or_default();
                    let df_expected = &expected_at_times[&time][*time_count];
                    *time_count += 1;
                    df_expected
                } else {
                    let df_expected = &expected_timeless[timeless_count];
                    timeless_count += 1;
                    df_expected
                };

                assert_eq!(*df_expected, df, "{store}");

                dfs_processed += 1;
            }

            let dfs_processed_expected = bundles_at_times.len();
            assert_eq!(dfs_processed_expected, dfs_processed);
        };

    // TODO(cmc): bring back some log_time scenarios

    // Unit ranges (ColorRGBA's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame1),
        [ColorRGBA::name(), Point2D::name()],
        &[
            (
                Some(frame0),
                &[
                    (ColorRGBA::name(), &bundle4_3),
                    (Point2D::name(), &bundle4_4),
                ],
            ), // timeless
            (
                Some(frame1),
                &[
                    (ColorRGBA::name(), &bundle1),
                    (Point2D::name(), &bundle4_4), // timeless
                ],
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame2, frame2),
        [ColorRGBA::name(), Point2D::name()],
        &[
            (
                Some(frame1),
                &[
                    (ColorRGBA::name(), &bundle1),
                    (Point2D::name(), &bundle4_4), // timeless
                ],
            ), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame3, frame3),
        [ColorRGBA::name(), Point2D::name()],
        &[
            (
                Some(frame2),
                &[(ColorRGBA::name(), &bundle1), (Point2D::name(), &bundle2)],
            ), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame4, frame4),
        [ColorRGBA::name(), Point2D::name()],
        &[
            (
                Some(frame3),
                &[(ColorRGBA::name(), &bundle1), (Point2D::name(), &bundle3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &bundle4_1), (Point2D::name(), &bundle3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &bundle4_2), (Point2D::name(), &bundle3)],
            ),
            (
                Some(frame4),
                &[
                    (ColorRGBA::name(), &bundle4_3),
                    (Point2D::name(), &bundle4_25),
                ], // !!!
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame5, frame5),
        [ColorRGBA::name(), Point2D::name()],
        &[
            (
                Some(frame4),
                &[
                    (ColorRGBA::name(), &bundle4_3),
                    (Point2D::name(), &bundle4_4),
                ], // !!!
            ), //
        ],
    );

    // Unit ranges (Point2D's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame1),
        [Point2D::name(), ColorRGBA::name()],
        &[
            (
                Some(frame0),
                &[
                    (Point2D::name(), &bundle4_4),
                    (ColorRGBA::name(), &bundle4_3),
                ],
            ), // timeless
        ],
    );
    assert_range_components(
        TimeRange::new(frame2, frame2),
        [Point2D::name(), ColorRGBA::name()],
        &[
            (
                Some(frame1),
                &[
                    (Point2D::name(), &bundle4_4), // timeless
                    (ColorRGBA::name(), &bundle1),
                ],
            ),
            (
                Some(frame2),
                &[(Point2D::name(), &bundle2), (ColorRGBA::name(), &bundle1)],
            ), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame3, frame3),
        [Point2D::name(), ColorRGBA::name()],
        &[
            (
                Some(frame2),
                &[(Point2D::name(), &bundle2), (ColorRGBA::name(), &bundle1)],
            ),
            (
                Some(frame3),
                &[(Point2D::name(), &bundle3), (ColorRGBA::name(), &bundle1)],
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame4, frame4),
        [Point2D::name(), ColorRGBA::name()],
        &[
            (
                Some(frame3),
                &[(Point2D::name(), &bundle3), (ColorRGBA::name(), &bundle1)],
            ),
            (
                Some(frame4),
                &[
                    (Point2D::name(), &bundle4_25),
                    (ColorRGBA::name(), &bundle4_2),
                ],
            ),
            (
                Some(frame4),
                &[
                    (Point2D::name(), &bundle4_4),
                    (ColorRGBA::name(), &bundle4_3),
                ],
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame5, frame5),
        [Point2D::name(), ColorRGBA::name()],
        &[
            (
                Some(frame4),
                &[
                    (Point2D::name(), &bundle4_4),
                    (ColorRGBA::name(), &bundle4_3),
                ],
            ), //
        ],
    );

    // Full range (ColorRGBA's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame5),
        [ColorRGBA::name(), Point2D::name()],
        &[
            (
                Some(frame0),
                &[
                    (ColorRGBA::name(), &bundle4_3),
                    (Point2D::name(), &bundle4_4),
                ],
            ), // timeless
            (
                Some(frame1),
                &[
                    (ColorRGBA::name(), &bundle1),
                    (Point2D::name(), &bundle4_4), // timeless
                ],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &bundle4_1), (Point2D::name(), &bundle3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &bundle4_2), (Point2D::name(), &bundle3)],
            ),
            (
                Some(frame4),
                &[
                    (ColorRGBA::name(), &bundle4_3),
                    (Point2D::name(), &bundle4_25),
                ], // !!!
            ),
        ],
    );

    // Full range (Point2D's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame5),
        [Point2D::name(), ColorRGBA::name()],
        &[
            (
                Some(frame0),
                &[
                    (Point2D::name(), &bundle4_4),
                    (ColorRGBA::name(), &bundle4_3),
                ],
            ), // timeless
            (
                Some(frame2),
                &[(Point2D::name(), &bundle2), (ColorRGBA::name(), &bundle1)],
            ),
            (
                Some(frame3),
                &[(Point2D::name(), &bundle3), (ColorRGBA::name(), &bundle1)],
            ),
            (
                Some(frame4),
                &[
                    (Point2D::name(), &bundle4_25),
                    (ColorRGBA::name(), &bundle4_2),
                ],
            ),
            (
                Some(frame4),
                &[
                    (Point2D::name(), &bundle4_4),
                    (ColorRGBA::name(), &bundle4_3),
                ],
            ),
        ],
    );

    // Infinite range (ColorRGBA's PoV)

    assert_range_components(
        TimeRange::new(TimeInt::MIN, TimeInt::MAX),
        [ColorRGBA::name(), Point2D::name()],
        &[
            (None, &[(ColorRGBA::name(), &bundle1)]),
            (
                None,
                &[(ColorRGBA::name(), &bundle4_1), (Point2D::name(), &bundle3)],
            ),
            (
                None,
                &[(ColorRGBA::name(), &bundle4_2), (Point2D::name(), &bundle3)],
            ),
            (
                None,
                &[
                    (ColorRGBA::name(), &bundle4_3),
                    (Point2D::name(), &bundle4_25),
                ], // !!!
            ),
            (
                Some(frame1),
                &[
                    (ColorRGBA::name(), &bundle1),
                    (Point2D::name(), &bundle4_4), // timeless
                ],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &bundle4_1), (Point2D::name(), &bundle3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &bundle4_2), (Point2D::name(), &bundle3)],
            ),
            (
                Some(frame4),
                &[
                    (ColorRGBA::name(), &bundle4_3),
                    (Point2D::name(), &bundle4_25),
                ], // !!!
            ),
        ],
    );

    // Infinite range (Point2D's PoV)

    assert_range_components(
        TimeRange::new(TimeInt::MIN, TimeInt::MAX),
        [Point2D::name(), ColorRGBA::name()],
        &[
            (
                None,
                &[(Point2D::name(), &bundle2), (ColorRGBA::name(), &bundle1)],
            ),
            (
                None,
                &[(Point2D::name(), &bundle3), (ColorRGBA::name(), &bundle1)],
            ),
            (
                None,
                &[
                    (Point2D::name(), &bundle4_25),
                    (ColorRGBA::name(), &bundle4_2),
                ],
            ),
            (
                None,
                &[
                    (Point2D::name(), &bundle4_4),
                    (ColorRGBA::name(), &bundle4_3),
                ],
            ),
            (
                Some(frame2),
                &[(Point2D::name(), &bundle2), (ColorRGBA::name(), &bundle1)],
            ),
            (
                Some(frame3),
                &[(Point2D::name(), &bundle3), (ColorRGBA::name(), &bundle1)],
            ),
            (
                Some(frame4),
                &[
                    (Point2D::name(), &bundle4_25),
                    (ColorRGBA::name(), &bundle4_2),
                ],
            ),
            (
                Some(frame4),
                &[
                    (Point2D::name(), &bundle4_4),
                    (ColorRGBA::name(), &bundle4_3),
                ],
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
            let cluster_comp = if let Some(idx) = bundle.find_component(&cluster_key) {
                Series::try_from((cluster_key.as_str(), bundle.components[idx].value_boxed()))
                    .unwrap()
            } else {
                let num_instances = bundle.num_instances(0).unwrap_or(0);
                Series::try_from((
                    cluster_key.as_str(),
                    wrap_in_listarray(
                        UInt64Array::from_vec((0..num_instances as u64).collect()).to_boxed(),
                    )
                    .to_boxed(),
                ))
                .unwrap()
            };

            let comp_idx = bundle.find_component(component).unwrap();
            let df = DataFrame::new(vec![
                cluster_comp,
                Series::try_from((
                    component.as_str(),
                    bundle.components[comp_idx].value_boxed(),
                ))
                .unwrap(),
            ])
            .unwrap();

            df.explode(df.get_column_names()).unwrap()
        })
        .reduce(|left, right| {
            left.outer_join(&right, [cluster_key.as_str()], [cluster_key.as_str()])
                .unwrap()
        })
        .unwrap_or_default();

    let df = polars_util::drop_all_nulls(&df, &cluster_key).unwrap();

    df.sort([cluster_key.as_str()], false).unwrap_or(df)
}

// --- GC ---

#[test]
fn gc() {
    init_logs();

    for config in re_arrow_store::test_util::all_configs() {
        let mut store = DataStore::new(InstanceKey::name(), config.clone());
        gc_impl(&mut store);
    }
}
fn gc_impl(store: &mut DataStore) {
    let mut rng = rand::thread_rng();

    for _ in 0..2 {
        let num_ents = 10;
        for i in 0..num_ents {
            let ent_path = EntityPath::from(format!("this/that/{i}"));

            let num_frames = rng.gen_range(0..=100);
            let frames = (0..num_frames).filter(|_| rand::thread_rng().gen());
            for frame_nr in frames {
                let num_instances = rng.gen_range(0..=1_000);
                let bundle = test_bundle!(ent_path @ [build_frame_nr(frame_nr.into())] => [
                    build_some_rects(num_instances),
                ]);
                store.insert(&bundle).unwrap();
            }
        }

        if let err @ Err(_) = store.sanity_check() {
            store.sort_indices_if_needed();
            eprintln!("{store}");
            err.unwrap();
        }
        _ = store.to_dataframe(); // simple way of checking that everything is still readable

        let msg_id_chunks = store.gc(
            GarbageCollectionTarget::DropAtLeastPercentage(1.0 / 3.0),
            Timeline::new("frame_nr", TimeType::Sequence),
            MsgId::name(),
        );

        let msg_ids = msg_id_chunks
            .iter()
            .flat_map(|chunk| arrow_array_deserialize_iterator::<Option<MsgId>>(&**chunk).unwrap())
            .map(Option::unwrap) // MsgId is always present
            .collect::<ahash::HashSet<_>>();

        for msg_id in &msg_ids {
            assert!(store.get_msg_metadata(msg_id).is_some());
        }

        store.clear_msg_metadata(&msg_ids);

        for msg_id in &msg_ids {
            assert!(store.get_msg_metadata(msg_id).is_none());
        }
    }
}

// ---

pub fn init_logs() {
    static INIT: AtomicBool = AtomicBool::new(false);

    if INIT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        re_log::setup_native_logging();
    }
}
