//! Straightforward high-level API tests.
//!
//! Testing & demonstrating expected usage of the datastore APIs, no funny stuff.

use itertools::Itertools;
use rand::Rng;
use re_data_store::{
    test_row,
    test_util::{init_logs, insert_table_with_retries, sanity_unwrap},
    DataStore, DataStoreConfig, DataStoreStats, GarbageCollectionOptions, GarbageCollectionTarget,
    LatestAtQuery, TimeInt, TimeRange,
};
use re_log_types::{build_frame_nr, DataRow, DataTable, EntityPath, TableId, TimeType, Timeline};
use re_types::datagen::{
    build_some_colors, build_some_instances, build_some_instances_from, build_some_positions2d,
};
use re_types::{
    components::{Color, InstanceKey, Position2D},
    testing::{build_some_large_structs, LargeStruct},
};
use re_types_core::{ComponentName, Loggable as _};

// --- LatestComponentsAt ---

#[test]
fn all_components() {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    // let frame0= TimeInt::from(0);
    let frame1 = TimeInt::from(1);
    let frame2 = TimeInt::from(2);
    let frame3 = TimeInt::from(3);
    let frame4 = TimeInt::from(4);

    let assert_latest_components_at =
        |store: &mut DataStore, ent_path: &EntityPath, expected: Option<&[ComponentName]>| {
            // Stress test save-to-disk & load-from-disk
            let mut store2 = DataStore::new(
                re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
                store.cluster_key(),
                store.config().clone(),
            );
            for table in store.to_data_tables(None) {
                insert_table_with_retries(&mut store2, &table);
            }

            // Stress test GC
            store2.gc(&GarbageCollectionOptions::gc_everything());
            for table in store.to_data_tables(None) {
                insert_table_with_retries(&mut store2, &table);
            }

            let store = store2;
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
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            DataStoreConfig {
                indexed_bucket_num_rows: u64::MAX,
                ..Default::default()
            },
        );
        let cluster_key = store.cluster_key();

        let components_a = &[
            Color::name(),       // added by test, timeless
            LargeStruct::name(), // added by test
            cluster_key,         // always here
        ];

        let components_b = &[
            Color::name(),       // added by test, timeless
            Position2D::name(),  // added by test
            LargeStruct::name(), // added by test
            cluster_key,         // always here
        ];

        let row = test_row!(ent_path @ [] => 2; [build_some_colors(2)]);
        store.insert_row(&row).unwrap();

        let row =
            test_row!(ent_path @ [build_frame_nr(frame1)] => 2; [build_some_large_structs(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row = test_row!(ent_path @ [
            build_frame_nr(frame2),
        ] => 2; [build_some_large_structs(2), build_some_positions2d(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_b));

        sanity_unwrap(&store);
    }

    // Tiny buckets, demonstrating the harder-to-reason-about cases.
    {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            DataStoreConfig {
                indexed_bucket_num_rows: 0,
                ..Default::default()
            },
        );
        let cluster_key = store.cluster_key();

        // ┌──────────┬─────────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ LargeStruct ┆ row_id ┆ insert_id ┆ instance │
        // ╞══════════╪═════════════╪════════╪═══════════╪══════════╡
        // │ 1        ┆ 1           ┆ 1      ┆ 1         ┆ 1        │
        // └──────────┴─────────────┴────────┴───────────┴──────────┘
        // ┌──────────┬─────────────┬─────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ LargeStruct ┆ point2d ┆ row_id ┆ insert_id ┆ instance │
        // ╞══════════╪═════════════╪═════════╪════════╪═══════════╪══════════╡
        // │ 2        ┆ -           ┆ -       ┆ 2      ┆ 2         ┆ 2        │
        // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
        // │ 3        ┆ -           ┆ 1       ┆ 3      ┆ 3         ┆ 1        │
        // └──────────┴─────────────┴─────────┴────────┴───────────┴──────────┘

        let components_a = &[
            Color::name(),       // added by test, timeless
            LargeStruct::name(), // added by test
            cluster_key,         // always here
        ];

        let components_b = &[
            Color::name(),       // added by test, timeless
            LargeStruct::name(), // ⚠ inherited before the buckets got split apart!
            Position2D::name(),  // added by test
            cluster_key,         // always here
        ];

        let row = test_row!(ent_path @ [] => 2; [build_some_colors(2)]);
        store.insert_row(&row).unwrap();

        let row =
            test_row!(ent_path @ [build_frame_nr(frame1)] => 2; [build_some_large_structs(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row = test_row!(ent_path @ [build_frame_nr(frame2)] => 2; [build_some_instances(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row = test_row!(ent_path @ [build_frame_nr(frame3)] => 2; [build_some_positions2d(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_b));

        sanity_unwrap(&store);
    }

    // Tiny buckets and tricky splits, demonstrating a case that is not only extremely hard to
    // reason about, it is technically incorrect.
    {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            DataStoreConfig {
                indexed_bucket_num_rows: 0,
                ..Default::default()
            },
        );
        let cluster_key = store.cluster_key();

        // ┌──────────┬─────────────┬─────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ LargeStruct ┆ point2d ┆ row_id ┆ insert_id ┆ instance │
        // ╞══════════╪═════════════╪═════════╪════════╪═══════════╪══════════╡
        // │ 1        ┆ -           ┆ 1       ┆ 4      ┆ 4         ┆ 1        │
        // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
        // │ 2        ┆ 1           ┆ -       ┆ 1      ┆ 1         ┆ 1        │
        // └──────────┴─────────────┴─────────┴────────┴───────────┴──────────┘
        // ┌──────────┬─────────────┬────────┬───────────┬──────────┐
        // │ frame_nr ┆ LargeStruct ┆ row_id ┆ insert_id ┆ instance │
        // ╞══════════╪═════════════╪════════╪═══════════╪══════════╡
        // │ 3        ┆ 2           ┆ 2      ┆ 2         ┆ 1        │
        // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
        // │ 4        ┆ 3           ┆ 3      ┆ 3         ┆ 1        │
        // └──────────┴─────────────┴────────┴───────────┴──────────┘

        let components_a = &[
            Color::name(),       // added by test, timeless
            LargeStruct::name(), // added by test
            cluster_key,         // always here
        ];

        let components_b = &[
            Color::name(),       // added by test, timeless
            Position2D::name(),  // added by test but not contained in the second bucket
            LargeStruct::name(), // added by test
            cluster_key,         // always here
        ];

        let row = test_row!(ent_path @ [] => 2; [build_some_colors(2)]);
        store.insert_row(&row).unwrap();

        let row =
            test_row!(ent_path @ [build_frame_nr(frame2)] => 2; [build_some_large_structs(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row =
            test_row!(ent_path @ [build_frame_nr(frame3)] => 2; [build_some_large_structs(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row =
            test_row!(ent_path @ [build_frame_nr(frame4)] => 2; [build_some_large_structs(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_a));

        let row = test_row!(ent_path @ [build_frame_nr(frame1)] => 2; [build_some_positions2d(2)]);
        store.insert_row(&row).unwrap();

        assert_latest_components_at(&mut store, &ent_path, Some(components_b));

        sanity_unwrap(&store);
    }
}

// --- LatestAt ---

#[test]
fn latest_at() {
    init_logs();

    for config in re_data_store::test_util::all_configs() {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        latest_at_impl(&mut store);
    }
}

fn latest_at_impl(store: &mut DataStore) {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    let frame0 = TimeInt::from(0);
    let frame1 = TimeInt::from(1);
    let frame2 = TimeInt::from(2);
    let frame3 = TimeInt::from(3);
    let frame4 = TimeInt::from(4);

    // helper to insert a table both as a temporal and timeless payload
    let insert_table = |store: &mut DataStore, table: &DataTable| {
        // insert temporal
        insert_table_with_retries(store, table);

        // insert timeless
        let mut table_timeless = table.clone();
        table_timeless.col_timelines = Default::default();
        insert_table_with_retries(store, &table_timeless);
    };

    let (instances1, colors1) = (build_some_instances(3), build_some_colors(3));
    let row1 = test_row!(ent_path @ [build_frame_nr(frame1)] => 3; [instances1.clone(), colors1]);

    let positions2 = build_some_positions2d(3);
    let row2 = test_row!(ent_path @ [build_frame_nr(frame2)] => 3; [instances1, positions2]);

    let points3 = build_some_positions2d(10);
    let row3 = test_row!(ent_path @ [build_frame_nr(frame3)] => 10; [points3]);

    let colors4 = build_some_colors(5);
    let row4 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [colors4]);

    insert_table(
        store,
        &DataTable::from_rows(
            TableId::new(),
            [row1.clone(), row2.clone(), row3.clone(), row4.clone()],
        ),
    );

    // Stress test save-to-disk & load-from-disk
    let mut store2 = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        store.cluster_key(),
        store.config().clone(),
    );
    for table in store.to_data_tables(None) {
        insert_table(&mut store2, &table);
    }
    // Stress test GC
    store2.gc(&GarbageCollectionOptions::gc_everything());
    for table in store.to_data_tables(None) {
        insert_table(&mut store2, &table);
    }
    let store = store2;

    sanity_unwrap(&store);

    let assert_latest_components = |frame_nr: TimeInt, rows: &[(ComponentName, &DataRow)]| {
        let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

        for (component_name, expected) in rows {
            let (_, _, cells) = store
                .latest_at::<1>(
                    &LatestAtQuery::new(timeline_frame_nr, frame_nr),
                    &ent_path,
                    *component_name,
                    &[*component_name],
                )
                .unwrap();

            let expected = expected
                .cells
                .iter()
                .filter(|cell| cell.component_name() == *component_name)
                .collect_vec();
            let actual = cells.iter().flatten().collect_vec();
            assert_eq!(expected, actual);
        }
    };

    // TODO(cmc): bring back some log_time scenarios

    assert_latest_components(
        frame0,
        &[(Color::name(), &row4), (Position2D::name(), &row3)], // timeless
    );
    assert_latest_components(
        frame1,
        &[
            (Color::name(), &row1),
            (Position2D::name(), &row3), // timeless
        ],
    );
    assert_latest_components(
        frame2,
        &[(Color::name(), &row1), (Position2D::name(), &row2)],
    );
    assert_latest_components(
        frame3,
        &[(Color::name(), &row1), (Position2D::name(), &row3)],
    );
    assert_latest_components(
        frame4,
        &[(Color::name(), &row4), (Position2D::name(), &row3)],
    );
}

// --- Range ---

// TODO: requires join semantics -> re_query

#[test]
fn range() {
    init_logs();

    for config in re_data_store::test_util::all_configs() {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        range_impl(&mut store);
    }
}

fn range_impl(store: &mut DataStore) {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    let frame1 = TimeInt::from(1);
    let frame2 = TimeInt::from(2);
    let frame3 = TimeInt::from(3);
    let frame4 = TimeInt::from(4);
    let frame5 = TimeInt::from(5);

    // helper to insert a row both as a temporal and timeless payload
    let insert = |store: &mut DataStore, row| {
        // insert temporal
        store.insert_row(row).unwrap();

        // insert timeless
        let mut row_timeless = (*row).clone().next();
        row_timeless.timepoint = Default::default();
        store.insert_row(&row_timeless).unwrap();
    };

    let insts1 = build_some_instances(3);
    let colors1 = build_some_colors(3);
    let row1 = test_row!(ent_path @ [build_frame_nr(frame1)] => 3; [insts1.clone(), colors1]);
    insert(store, &row1);

    let positions2 = build_some_positions2d(3);
    let row2 = test_row!(ent_path @ [build_frame_nr(frame2)] => 3; [insts1, positions2]);
    insert(store, &row2);

    let points3 = build_some_positions2d(10);
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

    let points4_25 = build_some_positions2d(5);
    let row4_25 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [insts4_2, points4_25]);
    insert(store, &row4_25);

    let insts4_3 = build_some_instances_from(30..35);
    let colors4_3 = build_some_colors(5);
    let row4_3 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [insts4_3.clone(), colors4_3]);
    insert(store, &row4_3);

    let points4_4 = build_some_positions2d(5);
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
            let mut store2 = DataStore::new(
                re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
                store.cluster_key(),
                store.config().clone(),
            );
            for table in store.to_data_tables(None) {
                insert_table_with_retries(&mut store2, &table);
            }
            store2.gc(&GarbageCollectionOptions::gc_everything());
            for table in store.to_data_tables(None) {
                insert_table_with_retries(&mut store2, &table);
            }
            let store = store2;

            // TODO
            // let mut expected_timeless = Vec::<DataFrame>::new();
            // let mut expected_at_times: IntMap<TimeInt, Vec<DataFrame>> = Default::default();
            //
            // for (time, rows) in rows_at_times {
            //     if let Some(time) = time {
            //         let dfs = expected_at_times.entry(*time).or_default();
            //         dfs.push(joint_df(store.cluster_key(), rows));
            //     } else {
            //         expected_timeless.push(joint_df(store.cluster_key(), rows));
            //     }
            // }
            //
            // let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
            //
            // store.sort_indices_if_needed(); // for assertions below
            //
            // let components = [InstanceKey::name(), components[0], components[1]];
            // let query = RangeQuery::new(timeline_frame_nr, time_range);
            // let dfs = polars_util::range_components(
            //     &store,
            //     &query,
            //     &ent_path,
            //     components[1],
            //     components,
            //     &JoinType::Outer,
            // );
            //
            // let mut dfs_processed = 0usize;
            // let mut timeless_count = 0usize;
            // let mut time_counters: IntMap<i64, usize> = Default::default();
            // for (time, df) in dfs.map(Result::unwrap) {
            //     let df_expected = if let Some(time) = time {
            //         let time_count = time_counters.entry(time.as_i64()).or_default();
            //         let df_expected = &expected_at_times[&time][*time_count];
            //         *time_count += 1;
            //         df_expected
            //     } else {
            //         let df_expected = &expected_timeless[timeless_count];
            //         timeless_count += 1;
            //         df_expected
            //     };
            //
            //     assert_eq!(*df_expected, df, "{store}");
            //
            //     dfs_processed += 1;
            // }
            //
            // let dfs_processed_expected = rows_at_times.len();
            // assert_eq!(dfs_processed_expected, dfs_processed);
        };

    // TODO(cmc): bring back some log_time scenarios

    // Unit ranges (Color's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame1),
        [Color::name(), Position2D::name()],
        &[
            (
                Some(frame1),
                &[
                    (Color::name(), &row1),
                    (Position2D::name(), &row4_4), // timeless
                ],
            ), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame2, frame2),
        [Color::name(), Position2D::name()],
        &[],
    );
    assert_range_components(
        TimeRange::new(frame3, frame3),
        [Color::name(), Position2D::name()],
        &[],
    );
    assert_range_components(
        TimeRange::new(frame4, frame4),
        [Color::name(), Position2D::name()],
        &[
            (
                Some(frame4),
                &[(Color::name(), &row4_1), (Position2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(Color::name(), &row4_2), (Position2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(Color::name(), &row4_3), (Position2D::name(), &row4_25)], // !!!
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame5, frame5),
        [Color::name(), Position2D::name()],
        &[],
    );

    // Unit ranges (Position2D's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame1),
        [Position2D::name(), Color::name()],
        &[],
    );
    assert_range_components(
        TimeRange::new(frame2, frame2),
        [Position2D::name(), Color::name()],
        &[
            (
                Some(frame2),
                &[(Position2D::name(), &row2), (Color::name(), &row1)],
            ), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame3, frame3),
        [Position2D::name(), Color::name()],
        &[
            (
                Some(frame3),
                &[(Position2D::name(), &row3), (Color::name(), &row1)],
            ), //
        ],
    );
    assert_range_components(
        TimeRange::new(frame4, frame4),
        [Position2D::name(), Color::name()],
        &[
            (
                Some(frame4),
                &[(Position2D::name(), &row4_25), (Color::name(), &row4_2)],
            ),
            (
                Some(frame4),
                &[(Position2D::name(), &row4_4), (Color::name(), &row4_3)],
            ),
        ],
    );
    assert_range_components(
        TimeRange::new(frame5, frame5),
        [Position2D::name(), Color::name()],
        &[],
    );

    // Full range (Color's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame5),
        [Color::name(), Position2D::name()],
        &[
            (
                Some(frame1),
                &[
                    (Color::name(), &row1),
                    (Position2D::name(), &row4_4), // timeless
                ],
            ),
            (
                Some(frame4),
                &[(Color::name(), &row4_1), (Position2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(Color::name(), &row4_2), (Position2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(Color::name(), &row4_3), (Position2D::name(), &row4_25)], // !!!
            ),
        ],
    );

    // Full range (Position2D's PoV)

    assert_range_components(
        TimeRange::new(frame1, frame5),
        [Position2D::name(), Color::name()],
        &[
            (
                Some(frame2),
                &[(Position2D::name(), &row2), (Color::name(), &row1)],
            ),
            (
                Some(frame3),
                &[(Position2D::name(), &row3), (Color::name(), &row1)],
            ),
            (
                Some(frame4),
                &[(Position2D::name(), &row4_25), (Color::name(), &row4_2)],
            ),
            (
                Some(frame4),
                &[(Position2D::name(), &row4_4), (Color::name(), &row4_3)],
            ),
        ],
    );

    // Infinite range (Color's PoV)

    assert_range_components(
        TimeRange::new(TimeInt::MIN, TimeInt::MAX),
        [Color::name(), Position2D::name()],
        &[
            (None, &[(Color::name(), &row1)]),
            (
                None,
                &[(Color::name(), &row4_1), (Position2D::name(), &row3)],
            ),
            (
                None,
                &[(Color::name(), &row4_2), (Position2D::name(), &row3)],
            ),
            (
                None,
                &[(Color::name(), &row4_3), (Position2D::name(), &row4_25)], // !!!
            ),
            (
                Some(frame1),
                &[
                    (Color::name(), &row1),
                    (Position2D::name(), &row4_4), // timeless
                ],
            ),
            (
                Some(frame4),
                &[(Color::name(), &row4_1), (Position2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(Color::name(), &row4_2), (Position2D::name(), &row3)],
            ),
            (
                Some(frame4),
                &[(Color::name(), &row4_3), (Position2D::name(), &row4_25)], // !!!
            ),
        ],
    );

    // Infinite range (Position2D's PoV)

    assert_range_components(
        TimeRange::new(TimeInt::MIN, TimeInt::MAX),
        [Position2D::name(), Color::name()],
        &[
            (None, &[(Position2D::name(), &row2), (Color::name(), &row1)]),
            (None, &[(Position2D::name(), &row3), (Color::name(), &row1)]),
            (
                None,
                &[(Position2D::name(), &row4_25), (Color::name(), &row4_2)],
            ),
            (
                None,
                &[(Position2D::name(), &row4_4), (Color::name(), &row4_3)],
            ),
            (
                Some(frame2),
                &[(Position2D::name(), &row2), (Color::name(), &row1)],
            ),
            (
                Some(frame3),
                &[(Position2D::name(), &row3), (Color::name(), &row1)],
            ),
            (
                Some(frame4),
                &[(Position2D::name(), &row4_25), (Color::name(), &row4_2)],
            ),
            (
                Some(frame4),
                &[(Position2D::name(), &row4_4), (Color::name(), &row4_3)],
            ),
        ],
    );
}

// --- Common helpers ---

// /// Given a list of rows, crafts a `latest_components`-looking dataframe.
// fn joint_df(cluster_key: ComponentName, rows: &[(ComponentName, &DataRow)]) -> DataFrame {
//     let df = rows
//         .iter()
//         .map(|(component, row)| {
//             let cluster_comp = if let Some(idx) = row.find_cell(&cluster_key) {
//                 Series::try_from((cluster_key.as_ref(), row.cells[idx].to_arrow_monolist()))
//                     .unwrap()
//             } else {
//                 let num_instances = row.num_instances();
//                 Series::try_from((
//                     cluster_key.as_ref(),
//                     DataCell::from_component::<InstanceKey>(0..num_instances.get() as u64)
//                         .to_arrow_monolist(),
//                 ))
//                 .unwrap()
//             };
//
//             let comp_idx = row.find_cell(component).unwrap();
//             let df = DataFrame::new(vec![
//                 cluster_comp,
//                 Series::try_from((
//                     component.as_ref(),
//                     row.cells[comp_idx]
//                         .to_arrow_monolist()
//                         .as_ref()
//                         .clean_for_polars(),
//                 ))
//                 .unwrap(),
//             ])
//             .unwrap();
//
//             df.explode(df.get_column_names()).unwrap()
//         })
//         .reduce(|left, right| {
//             left.outer_join(&right, [cluster_key.as_ref()], [cluster_key.as_ref()])
//                 .unwrap()
//         })
//         .unwrap_or_default();
//
//     let df = polars_util::drop_all_nulls(&df, &cluster_key).unwrap();
//
//     df.sort([cluster_key.as_ref()], false).unwrap_or(df)
// }

// --- GC ---

#[test]
fn gc() {
    init_logs();

    for config in re_data_store::test_util::all_configs() {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
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
                    build_some_large_structs(num_instances as _),
                ]);
                store.insert_row(&row).unwrap();
            }
        }

        sanity_unwrap(store);
        // TODO
        // _ = store.to_dataframe(); // simple way of checking that everything is still readable

        let stats = DataStoreStats::from_store(store);

        let (store_events, stats_diff) = store.gc(&GarbageCollectionOptions {
            target: GarbageCollectionTarget::DropAtLeastFraction(1.0 / 3.0),
            gc_timeless: false,
            protect_latest: 0,
            purge_empty_tables: false,
            dont_protect: Default::default(),
            enable_batching: false,
            time_budget: std::time::Duration::MAX,
        });
        for event in store_events {
            assert!(store.get_msg_metadata(&event.row_id).is_none());
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

#[test]
fn protected_gc() {
    init_logs();

    for config in re_data_store::test_util::all_configs() {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        protected_gc_impl(&mut store);
    }
}

// TODO: rewrite that one so it doesn't require join semantics

fn protected_gc_impl(store: &mut DataStore) {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    let frame0 = TimeInt::from(0);
    let frame1 = TimeInt::from(1);
    let frame2 = TimeInt::from(2);
    let frame3 = TimeInt::from(3);
    let frame4 = TimeInt::from(4);

    let (instances1, colors1) = (build_some_instances(3), build_some_colors(3));
    let row1 = test_row!(ent_path @ [build_frame_nr(frame1)] => 3; [instances1.clone(), colors1]);

    let positions2 = build_some_positions2d(3);
    let row2 = test_row!(ent_path @ [build_frame_nr(frame2)] => 3; [instances1, positions2]);

    let points3 = build_some_positions2d(10);
    let row3 = test_row!(ent_path @ [build_frame_nr(frame3)] => 10; [points3]);

    let colors4 = build_some_colors(5);
    let row4 = test_row!(ent_path @ [build_frame_nr(frame4)] => 5; [colors4]);

    store.insert_row(&row1).unwrap();
    store.insert_row(&row2).unwrap();
    store.insert_row(&row3).unwrap();
    store.insert_row(&row4).unwrap();

    // Re-insert row1 and row2 as timeless data as well
    let mut table_timeless =
        DataTable::from_rows(TableId::new(), [row1.clone().next(), row2.clone().next()]);
    table_timeless.col_timelines = Default::default();
    insert_table_with_retries(store, &table_timeless);

    store.gc(&GarbageCollectionOptions {
        target: GarbageCollectionTarget::Everything,
        gc_timeless: true,
        protect_latest: 1,
        purge_empty_tables: true,
        dont_protect: Default::default(),
        enable_batching: false,
        time_budget: std::time::Duration::MAX,
    });

    let assert_latest_components = |frame_nr: TimeInt, rows: &[(ComponentName, &DataRow)]| {
        let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
        let components_all = &[Color::name(), Position2D::name()];

        // TODO
        // let df = polars_util::latest_components(
        //     store,
        //     &LatestAtQuery::new(timeline_frame_nr, frame_nr),
        //     &ent_path,
        //     components_all,
        //     &JoinType::Outer,
        // )
        // .unwrap();
        //
        // let df_expected = joint_df(store.cluster_key(), rows);
        //
        // store.sort_indices_if_needed();
        // assert_eq!(df_expected, df, "{store}");
    };

    // The timeless data was preserved
    assert_latest_components(
        frame0,
        &[(Color::name(), &row1), (Position2D::name(), &row2)], // timeless
    );

    //
    assert_latest_components(
        frame3,
        &[
            (Color::name(), &row1),      // timeless
            (Position2D::name(), &row3), // protected
        ],
    );

    assert_latest_components(
        frame4,
        &[
            (Color::name(), &row4),      //protected
            (Position2D::name(), &row3), // protected
        ],
    );
}

#[test]
fn protected_gc_clear() {
    init_logs();

    for config in re_data_store::test_util::all_configs() {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        protected_gc_clear_impl(&mut store);
    }
}

// TODO: rewrite that one so it doesn't require join semantics

fn protected_gc_clear_impl(store: &mut DataStore) {
    init_logs();

    let ent_path = EntityPath::from("this/that");

    let frame0 = TimeInt::from(0);
    let frame1 = TimeInt::from(1);
    let frame2 = TimeInt::from(2);
    let frame3 = TimeInt::from(3);
    let frame4 = TimeInt::from(4);

    let (instances1, colors1) = (build_some_instances(3), build_some_colors(3));
    let row1 = test_row!(ent_path @ [build_frame_nr(frame1)] => 3; [instances1.clone(), colors1]);

    let positions2 = build_some_positions2d(3);
    let row2 = test_row!(ent_path @ [build_frame_nr(frame2)] => 3; [instances1, positions2]);

    let colors2 = build_some_colors(0);
    let row3 = test_row!(ent_path @ [build_frame_nr(frame3)] => 0; [colors2]);

    let points4 = build_some_positions2d(0);
    let row4 = test_row!(ent_path @ [build_frame_nr(frame4)] => 0; [points4]);

    // Insert the 3 rows as timeless
    let mut table_timeless =
        DataTable::from_rows(TableId::new(), [row1.clone(), row2.clone(), row3.clone()]);
    table_timeless.col_timelines = Default::default();
    insert_table_with_retries(store, &table_timeless);

    store.gc(&GarbageCollectionOptions {
        target: GarbageCollectionTarget::Everything,
        gc_timeless: true,
        protect_latest: 1,
        purge_empty_tables: true,
        dont_protect: Default::default(),
        enable_batching: false,
        time_budget: std::time::Duration::MAX,
    });

    let assert_latest_components = |frame_nr: TimeInt, rows: &[(ComponentName, &DataRow)]| {
        let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
        let components_all = &[Color::name(), Position2D::name()];

        // TODO
        // let df = polars_util::latest_components(
        //     store,
        //     &LatestAtQuery::new(timeline_frame_nr, frame_nr),
        //     &ent_path,
        //     components_all,
        //     &JoinType::Outer,
        // )
        // .unwrap();
        //
        // let df_expected = joint_df(store.cluster_key(), rows);
        //
        // store.sort_indices_if_needed();
        // assert_eq!(df_expected, df, "{store}");
    };

    // Only points are preserved, since colors were cleared and then GC'd
    assert_latest_components(
        frame0,
        &[(Color::name(), &row3), (Position2D::name(), &row2)],
    );

    // Only the 2 rows should remain in the table
    let stats = DataStoreStats::from_store(store);
    assert_eq!(stats.timeless.num_rows, 2);

    // Now erase points and GC again
    let mut table_timeless = DataTable::from_rows(TableId::new(), [row4]);
    table_timeless.col_timelines = Default::default();
    insert_table_with_retries(store, &table_timeless);

    store.gc(&GarbageCollectionOptions {
        target: GarbageCollectionTarget::Everything,
        gc_timeless: true,
        protect_latest: 1,
        purge_empty_tables: true,
        dont_protect: Default::default(),
        enable_batching: false,
        time_budget: std::time::Duration::MAX,
    });

    // No rows should remain because the table should have been purged
    let stats = DataStoreStats::from_store(store);
    assert_eq!(stats.timeless.num_rows, 0);
}
