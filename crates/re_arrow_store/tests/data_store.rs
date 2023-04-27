#![cfg(feature = "polars")]

//! Straightforward high-level API tests.
//!
//! Testing & demonstrating expected usage of the datastore APIs, no funny stuff.

use std::sync::atomic::{AtomicBool, Ordering};

use nohash_hasher::IntMap;
use polars_core::{prelude::*, series::Series};
use polars_ops::prelude::DataFrameJoinOps;
use rand::Rng;
use re_arrow_store::{
    polars_util, test_row, test_util::sanity_unwrap, DataStore, DataStoreConfig, DataStoreStats,
    GarbageCollectionTarget, LatestAtQuery, RangeQuery, TimeInt, TimeRange,
};
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey, Point2D, Rect2D},
    datagen::{
        build_frame_nr, build_some_colors, build_some_instances, build_some_instances_from,
        build_some_point2d, build_some_rects,
    },
    Component as _, ComponentName, DataCell, DataRow, DataTable, EntityPath, TableId, TimeType,
    Timeline,
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
            // Stress test save-to-disk & load-from-disk
            let mut store2 = DataStore::new(store.cluster_key(), store.config().clone());
            for table in store.to_data_tables(None) {
                store2.insert_table(&table).unwrap();
            }

            // Stress test GC
            store2.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0));
            for table in store.to_data_tables(None) {
                store2.insert_table(&table).unwrap();
            }

            let mut store = store2;
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
                indexed_bucket_num_rows: u64::MAX,
                ..Default::default()
            },
        );
        let cluster_key = store.cluster_key();

        let components_a = &[
            ColorRGBA::name(), // added by test, timeless
            Rect2D::name(),    // added by test
            cluster_key,       // always here
        ];

        let components_b = &[
            ColorRGBA::name(), // added by test, timeless
            Point2D::name(),   // added by test
            Rect2D::name(),    // added by test
            cluster_key,       // always here
        ];

        let row = test_row!(ent_path @ [] => 2; [build_some_colors(2)]);
        store.insert_row(&row).unwrap();

        let row = test_row!(ent_path @ [build_frame_nr(frame1)] => 2; [build_some_rects(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row = test_row!(ent_path @ [
            build_frame_nr(frame2),
        ] => 2; [build_some_rects(2), build_some_point2d(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_b));

        sanity_unwrap(&mut store);
    }

    // Tiny buckets, demonstrating the harder-to-reason-about cases.
    {
        let mut store = DataStore::new(
            InstanceKey::name(),
            DataStoreConfig {
                indexed_bucket_num_rows: 0,
                ..Default::default()
            },
        );
        let cluster_key = store.cluster_key();

        // ┌──────────┬────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ rect2d ┆ row_id ┆ insert_id ┆ instance │
        // ╞══════════╪════════╪════════╪═══════════╪══════════╡
        // │ 1        ┆ 1      ┆ 1      ┆ 1         ┆ 1        │
        // └──────────┴────────┴────────┴───────────┴──────────┘
        // ┌──────────┬────────┬─────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ rect2d ┆ point2d ┆ row_id ┆ insert_id ┆ instance │
        // ╞══════════╪════════╪═════════╪════════╪═══════════╪══════════╡
        // │ 2        ┆ -      ┆ -       ┆ 2      ┆ 2         ┆ 2        │
        // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
        // │ 3        ┆ -      ┆ 1       ┆ 3      ┆ 3         ┆ 1        │
        // └──────────┴────────┴─────────┴────────┴───────────┴──────────┘

        let components_a = &[
            ColorRGBA::name(), // added by test, timeless
            Rect2D::name(),    // added by test
            cluster_key,       // always here
        ];

        let components_b = &[
            ColorRGBA::name(), // added by test, timeless
            Rect2D::name(),    // ⚠ inherited before the buckets got split apart!
            Point2D::name(),   // added by test
            cluster_key,       // always here
        ];

        let row = test_row!(ent_path @ [] => 2; [build_some_colors(2)]);
        store.insert_row(&row).unwrap();

        let row = test_row!(ent_path @ [build_frame_nr(frame1)] => 2; [build_some_rects(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row = test_row!(ent_path @ [build_frame_nr(frame2)] => 2; [build_some_instances(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row = test_row!(ent_path @ [build_frame_nr(frame3)] => 2; [build_some_point2d(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_b));

        sanity_unwrap(&mut store);
    }

    // Tiny buckets and tricky splits, demonstrating a case that is not only extremely hard to
    // reason about, it is technically incorrect.
    {
        let mut store = DataStore::new(
            InstanceKey::name(),
            DataStoreConfig {
                indexed_bucket_num_rows: 0,
                ..Default::default()
            },
        );
        let cluster_key = store.cluster_key();

        // ┌──────────┬────────┬─────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ rect2d ┆ point2d ┆ row_id ┆ insert_id ┆ instance │
        // ╞══════════╪════════╪═════════╪════════╪═══════════╪══════════╡
        // │ 1        ┆ -      ┆ 1       ┆ 4      ┆ 4         ┆ 1        │
        // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
        // │ 2        ┆ 1      ┆ -       ┆ 1      ┆ 1         ┆ 1        │
        // └──────────┴────────┴─────────┴────────┴───────────┴──────────┘
        // ┌──────────┬────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ rect2d ┆ row_id ┆ insert_id ┆ instance │
        // ╞══════════╪════════╪════════╪═══════════╪══════════╡
        // │ 3        ┆ 2      ┆ 2      ┆ 2         ┆ 1        │
        // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
        // │ 4        ┆ 3      ┆ 3      ┆ 3         ┆ 1        │
        // └──────────┴────────┴────────┴───────────┴──────────┘

        let components_a = &[
            ColorRGBA::name(), // added by test, timeless
            Rect2D::name(),    // added by test
            cluster_key,       // always here
        ];

        let components_b = &[
            ColorRGBA::name(), // added by test, timeless
            Point2D::name(),   // added by test but not contained in the second bucket
            Rect2D::name(),    // added by test
            cluster_key,       // always here
        ];

        let row = test_row!(ent_path @ [] => 2; [build_some_colors(2)]);
        store.insert_row(&row).unwrap();

        let row = test_row!(ent_path @ [build_frame_nr(frame2)] => 2; [build_some_rects(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row = test_row!(ent_path @ [build_frame_nr(frame3)] => 2; [build_some_rects(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row = test_row!(ent_path @ [build_frame_nr(frame4)] => 2; [build_some_rects(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row = test_row!(ent_path @ [build_frame_nr(frame1)] => 2; [build_some_point2d(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_b));

        sanity_unwrap(&mut store);
    }
}

// --- LatestAt ---

#[test]
fn latest_at() {
    init_logs();

    for config in re_arrow_store::test_util::all_configs() {
        let mut store = DataStore::new(InstanceKey::name(), config.clone());
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

    // helper to insert a table both as a temporal and timeless payload
    let insert_table = |store: &mut DataStore, table: &DataTable| {
        // insert temporal
        store.insert_table(table).unwrap();

        // insert timeless
        let mut table_timeless = table.clone();
        table_timeless.col_timelines = Default::default();
        store.insert_table(&table_timeless).unwrap();
    };

    let (instances1, colors1) = (build_some_instances(3), build_some_colors(3));
    let row1 = test_row!(ent_path @ [build_frame_nr(frame1)] => 3; [instances1.clone(), colors1]);

    let points2 = build_some_point2d(3);
    let row2 = test_row!(ent_path @ [build_frame_nr(frame2)] => 3; [instances1, points2]);

    let points3 = build_some_point2d(10);
    let row3 = test_row!(ent_path @ [build_frame_nr(frame3)] => 10; [points3]);

    let colors4 = build_some_colors(5);
    let row4 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [colors4]);

    insert_table(
        store,
        &DataTable::from_rows(
            TableId::random(),
            [row1.clone(), row2.clone(), row3.clone(), row4.clone()],
        ),
    );

    // Stress test save-to-disk & load-from-disk
    let mut store2 = DataStore::new(store.cluster_key(), store.config().clone());
    for table in store.to_data_tables(None) {
        store2.insert_table(&table).unwrap();
    }
    // Stress test GC
    store2.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0));
    for table in store.to_data_tables(None) {
        store2.insert_table(&table).unwrap();
    }
    let mut store = store2;

    sanity_unwrap(&mut store);

    let mut assert_latest_components = |frame_nr: TimeInt, rows: &[(ComponentName, &DataRow)]| {
        let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
        let components_all = &[ColorRGBA::name(), Point2D::name()];

        let df = polars_util::latest_components(
            &store,
            &LatestAtQuery::new(timeline_frame_nr, frame_nr),
            &ent_path,
            components_all,
            &JoinType::Outer,
        )
        .unwrap();

        let df_expected = joint_df(store.cluster_key(), rows);

        store.sort_indices_if_needed();
        assert_eq!(df_expected, df, "{store}");
    };

    // TODO(cmc): bring back some log_time scenarios

    assert_latest_components(
        frame0,
        &[(ColorRGBA::name(), &row4), (Point2D::name(), &row3)], // timeless
    );
    assert_latest_components(
        frame1,
        &[
            (ColorRGBA::name(), &row1),
            (Point2D::name(), &row3), // timeless
        ],
    );
    assert_latest_components(
        frame2,
        &[(ColorRGBA::name(), &row1), (Point2D::name(), &row2)],
    );
    assert_latest_components(
        frame3,
        &[(ColorRGBA::name(), &row1), (Point2D::name(), &row3)],
    );
    assert_latest_components(
        frame4,
        &[(ColorRGBA::name(), &row4), (Point2D::name(), &row3)],
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

    // helper to insert a row both as a temporal and timeless payload
    let insert = |store: &mut DataStore, row| {
        // insert temporal
        store.insert_row(row).unwrap();

        // insert timeless
        let mut row_timeless = (*row).clone();
        row_timeless.timepoint = Default::default();
        store.insert_row(&row_timeless).unwrap();
    };

    let insts1 = build_some_instances(3);
    let colors1 = build_some_colors(3);
    let row1 = test_row!(ent_path @ [build_frame_nr(frame1)] => 3; [insts1.clone(), colors1]);
    insert(store, &row1);

    let points2 = build_some_point2d(3);
    let row2 = test_row!(ent_path @ [build_frame_nr(frame2)] => 3; [insts1, points2]);
    insert(store, &row2);

    let points3 = build_some_point2d(10);
    let row3 = test_row!(ent_path @ [build_frame_nr(frame3)] => 10; [points3]);
    insert(store, &row3);

    let insts4_1 = build_some_instances_from(20..25);
    let colors4_1 = build_some_colors(5);
    let row4_1 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [insts4_1, colors4_1]);
    insert(store, &row4_1);

    let insts4_2 = build_some_instances_from(25..30);
    let colors4_2 = build_some_colors(5);
    let row4_2 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [insts4_2.clone(), colors4_2]);
    insert(store, &row4_2);

    let points4_25 = build_some_point2d(5);
    let row4_25 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [insts4_2, points4_25]);
    insert(store, &row4_25);

    let insts4_3 = build_some_instances_from(30..35);
    let colors4_3 = build_some_colors(5);
    let row4_3 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [insts4_3.clone(), colors4_3]);
    insert(store, &row4_3);

    let points4_4 = build_some_point2d(5);
    let row4_4 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [insts4_3, points4_4]);
    insert(store, &row4_4);

    sanity_unwrap(store);

    // Each entry in `rows_at_times` corresponds to a dataframe that's expected to be returned
    // by the range query.
    // A single timepoint might have several of those! That's one of the behaviors specific to
    // range queries.
    #[allow(clippy::type_complexity)]
    let assert_range_components =
        |time_range: TimeRange,
         components: [ComponentName; 2],
         rows_at_times: &[(Option<TimeInt>, &[(ComponentName, &DataRow)])]| {
            // Stress test save-to-disk & load-from-disk
            let mut store2 = DataStore::new(store.cluster_key(), store.config().clone());
            for table in store.to_data_tables(None) {
                store2.insert_table(&table).unwrap();
            }
            store2.wipe_timeless_data();
            store2.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0));
            for table in store.to_data_tables(None) {
                store2.insert_table(&table).unwrap();
            }
            let mut store = store2;

            let mut expected_timeless = Vec::<DataFrame>::new();
            let mut expected_at_times: IntMap<TimeInt, Vec<DataFrame>> = Default::default();

            for (time, rows) in rows_at_times {
                if let Some(time) = time {
                    let dfs = expected_at_times.entry(*time).or_default();
                    dfs.push(joint_df(store.cluster_key(), rows));
                } else {
                    expected_timeless.push(joint_df(store.cluster_key(), rows));
                }
            }

            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

            store.sort_indices_if_needed(); // for assertions below

            let components = [InstanceKey::name(), components[0], components[1]];
            let query = RangeQuery::new(timeline_frame_nr, time_range);
            let dfs = polars_util::range_components(
                &store,
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

            let dfs_processed_expected = rows_at_times.len();
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
                &[(ColorRGBA::name(), &row4_3), (Point2D::name(), &row4_4)],
            ), // timeless
            (
                Some(frame1),
                &[
                    (ColorRGBA::name(), &row1),
                    (Point2D::name(), &row4_4), // timeless
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
                    (ColorRGBA::name(), &row1),
                    (Point2D::name(), &row4_4), // timeless
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
                &[(ColorRGBA::name(), &row1), (Point2D::name(), &row2)],
            ), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame4, frame4),
        [ColorRGBA::name(), Point2D::name()],
        &[
            (
                Some(frame3),
                &[(ColorRGBA::name(), &row1), (Point2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &row4_1), (Point2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &row4_2), (Point2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &row4_3), (Point2D::name(), &row4_25)], // !!!
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame5, frame5),
        [ColorRGBA::name(), Point2D::name()],
        &[
            (
                Some(frame4),
                &[(ColorRGBA::name(), &row4_3), (Point2D::name(), &row4_4)], // !!!
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
                &[(Point2D::name(), &row4_4), (ColorRGBA::name(), &row4_3)],
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
                    (Point2D::name(), &row4_4), // timeless
                    (ColorRGBA::name(), &row1),
                ],
            ),
            (
                Some(frame2),
                &[(Point2D::name(), &row2), (ColorRGBA::name(), &row1)],
            ), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame3, frame3),
        [Point2D::name(), ColorRGBA::name()],
        &[
            (
                Some(frame2),
                &[(Point2D::name(), &row2), (ColorRGBA::name(), &row1)],
            ),
            (
                Some(frame3),
                &[(Point2D::name(), &row3), (ColorRGBA::name(), &row1)],
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame4, frame4),
        [Point2D::name(), ColorRGBA::name()],
        &[
            (
                Some(frame3),
                &[(Point2D::name(), &row3), (ColorRGBA::name(), &row1)],
            ),
            (
                Some(frame4),
                &[(Point2D::name(), &row4_25), (ColorRGBA::name(), &row4_2)],
            ),
            (
                Some(frame4),
                &[(Point2D::name(), &row4_4), (ColorRGBA::name(), &row4_3)],
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame5, frame5),
        [Point2D::name(), ColorRGBA::name()],
        &[
            (
                Some(frame4),
                &[(Point2D::name(), &row4_4), (ColorRGBA::name(), &row4_3)],
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
                &[(ColorRGBA::name(), &row4_3), (Point2D::name(), &row4_4)],
            ), // timeless
            (
                Some(frame1),
                &[
                    (ColorRGBA::name(), &row1),
                    (Point2D::name(), &row4_4), // timeless
                ],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &row4_1), (Point2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &row4_2), (Point2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &row4_3), (Point2D::name(), &row4_25)], // !!!
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
                &[(Point2D::name(), &row4_4), (ColorRGBA::name(), &row4_3)],
            ), // timeless
            (
                Some(frame2),
                &[(Point2D::name(), &row2), (ColorRGBA::name(), &row1)],
            ),
            (
                Some(frame3),
                &[(Point2D::name(), &row3), (ColorRGBA::name(), &row1)],
            ),
            (
                Some(frame4),
                &[(Point2D::name(), &row4_25), (ColorRGBA::name(), &row4_2)],
            ),
            (
                Some(frame4),
                &[(Point2D::name(), &row4_4), (ColorRGBA::name(), &row4_3)],
            ),
        ],
    );

    // Infinite range (ColorRGBA's PoV)

    assert_range_components(
        TimeRange::new(TimeInt::MIN, TimeInt::MAX),
        [ColorRGBA::name(), Point2D::name()],
        &[
            (None, &[(ColorRGBA::name(), &row1)]),
            (
                None,
                &[(ColorRGBA::name(), &row4_1), (Point2D::name(), &row3)],
            ),
            (
                None,
                &[(ColorRGBA::name(), &row4_2), (Point2D::name(), &row3)],
            ),
            (
                None,
                &[(ColorRGBA::name(), &row4_3), (Point2D::name(), &row4_25)], // !!!
            ),
            (
                Some(frame1),
                &[
                    (ColorRGBA::name(), &row1),
                    (Point2D::name(), &row4_4), // timeless
                ],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &row4_1), (Point2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &row4_2), (Point2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(ColorRGBA::name(), &row4_3), (Point2D::name(), &row4_25)], // !!!
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
                &[(Point2D::name(), &row2), (ColorRGBA::name(), &row1)],
            ),
            (
                None,
                &[(Point2D::name(), &row3), (ColorRGBA::name(), &row1)],
            ),
            (
                None,
                &[(Point2D::name(), &row4_25), (ColorRGBA::name(), &row4_2)],
            ),
            (
                None,
                &[(Point2D::name(), &row4_4), (ColorRGBA::name(), &row4_3)],
            ),
            (
                Some(frame2),
                &[(Point2D::name(), &row2), (ColorRGBA::name(), &row1)],
            ),
            (
                Some(frame3),
                &[(Point2D::name(), &row3), (ColorRGBA::name(), &row1)],
            ),
            (
                Some(frame4),
                &[(Point2D::name(), &row4_25), (ColorRGBA::name(), &row4_2)],
            ),
            (
                Some(frame4),
                &[(Point2D::name(), &row4_4), (ColorRGBA::name(), &row4_3)],
            ),
        ],
    );
}

// --- Common helpers ---

/// Given a list of rows, crafts a `latest_components`-looking dataframe.
fn joint_df(cluster_key: ComponentName, rows: &[(ComponentName, &DataRow)]) -> DataFrame {
    let df = rows
        .iter()
        .map(|(component, row)| {
            let cluster_comp = if let Some(idx) = row.find_cell(&cluster_key) {
                Series::try_from((cluster_key.as_str(), row.cells[idx].to_arrow_monolist()))
                    .unwrap()
            } else {
                let num_instances = row.num_instances();
                Series::try_from((
                    cluster_key.as_str(),
                    DataCell::from_component::<InstanceKey>(0..num_instances as u64)
                        .to_arrow_monolist(),
                ))
                .unwrap()
            };

            let comp_idx = row.find_cell(component).unwrap();
            let df = DataFrame::new(vec![
                cluster_comp,
                Series::try_from((component.as_str(), row.cells[comp_idx].to_arrow_monolist()))
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
                let row = test_row!(ent_path @ [
                    build_frame_nr(frame_nr.into())
                ] => num_instances; [
                    build_some_rects(num_instances as _),
                ]);
                store.insert_row(&row).unwrap();
            }
        }

        sanity_unwrap(store);
        _ = store.to_dataframe(); // simple way of checking that everything is still readable

        let stats = DataStoreStats::from_store(store);

        let (row_ids, stats_diff) =
            store.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0 / 3.0));
        for row_id in &row_ids {
            assert!(store.get_msg_metadata(row_id).is_none());
        }

        // NOTE: only temporal data and row metadata get purged!
        let num_bytes_dropped =
            (stats_diff.temporal.num_bytes + stats_diff.metadata_registry.num_bytes) as f64;
        let num_bytes_dropped_expected_min =
            (stats.temporal.num_bytes + stats.metadata_registry.num_bytes) as f64 * 0.95 / 3.0;
        let num_bytes_dropped_expected_max =
            (stats.temporal.num_bytes + stats.metadata_registry.num_bytes) as f64 * 1.05 / 3.0;
        assert!(
            num_bytes_dropped_expected_min <= num_bytes_dropped
                && num_bytes_dropped <= num_bytes_dropped_expected_max,
            "{} <= {} <= {}",
            re_format::format_bytes(num_bytes_dropped_expected_min),
            re_format::format_bytes(num_bytes_dropped),
            re_format::format_bytes(num_bytes_dropped_expected_max),
        );
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
