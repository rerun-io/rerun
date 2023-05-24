//! Dumping a datastore to log messages and back.

use std::sync::atomic::{AtomicBool, Ordering};

use itertools::Itertools;
use re_arrow_store::{
    test_row, test_util::sanity_unwrap, DataStore, DataStoreStats, GarbageCollectionTarget,
    TimeInt, TimeRange, Timeline,
};
use re_log_types::{
    component_types::InstanceKey,
    datagen::{
        build_frame_nr, build_log_time, build_some_colors, build_some_instances, build_some_point2d,
    },
    Component as _, DataTable, EntityPath, TableId,
};

// --- Dump ---

#[test]
fn data_store_dump() {
    init_logs();

    for mut config in re_arrow_store::test_util::all_configs() {
        // NOTE: insert IDs aren't serialized and can be different across runs.
        config.store_insert_ids = false;

        let mut store1 = DataStore::new(InstanceKey::name(), config.clone());
        let mut store2 = DataStore::new(InstanceKey::name(), config.clone());
        let mut store3 = DataStore::new(InstanceKey::name(), config.clone());

        data_store_dump_impl(&mut store1, &mut store2, &mut store3);

        // stress-test GC impl
        store1.wipe_timeless_data();
        store1.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0));
        store2.wipe_timeless_data();
        store2.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0));
        store3.wipe_timeless_data();
        store3.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0));

        data_store_dump_impl(&mut store1, &mut store2, &mut store3);
    }
}

fn data_store_dump_impl(store1: &mut DataStore, store2: &mut DataStore, store3: &mut DataStore) {
    // helper to insert a table both as a temporal and timeless payload
    let insert_table = |store: &mut DataStore, table: &DataTable| {
        // insert temporal
        store.insert_table(table).unwrap();

        // insert timeless
        let mut table_timeless = table.clone();
        table_timeless.col_timelines = Default::default();
        store.insert_table(&table_timeless).unwrap();
    };

    let ent_paths = ["this/that", "other", "yet/another/one"];
    let tables = ent_paths
        .iter()
        .map(|ent_path| create_insert_table(*ent_path))
        .collect_vec();

    // Fill the first store.
    for table in &tables {
        insert_table(store1, table);
    }
    sanity_unwrap(store1);

    // Dump the first store into the second one.
    for table in store1.to_data_tables(None) {
        store2.insert_table(&table).unwrap();
    }
    sanity_unwrap(store2);

    // Dump the second store into the third one.
    for table in store2.to_data_tables(None) {
        store3.insert_table(&table).unwrap();
    }
    sanity_unwrap(store3);

    #[cfg(feature = "polars")]
    {
        let store1_df = store1.to_dataframe();
        let store2_df = store2.to_dataframe();
        let store3_df = store3.to_dataframe();
        assert!(
            store1_df == store2_df,
            "First & second stores differ:\n{store1_df}\n{store2_df}"
        );
        assert!(
            store1_df == store3_df,
            "First & third stores differ:\n{store1_df}\n{store3_df}"
        );
    }

    let store1_stats = DataStoreStats::from_store(store1);
    let store2_stats = DataStoreStats::from_store(store2);
    let store3_stats = DataStoreStats::from_store(store3);
    assert!(
        store1_stats.temporal.num_bytes <= store2_stats.temporal.num_bytes
            && store1_stats.timeless.num_bytes <= store2_stats.timeless.num_bytes,
        "First store should have <= amount of data of second store:\n\
            {store1_stats:#?}\n{store2_stats:#?}"
    );
    assert!(
        store2_stats.temporal.num_bytes <= store3_stats.temporal.num_bytes
            && store2_stats.timeless.num_bytes <= store3_stats.timeless.num_bytes,
        "Second store should have <= amount of data of third store:\n\
            {store2_stats:#?}\n{store3_stats:#?}"
    );
}

// --- Time-based filtering ---

#[test]
fn data_store_dump_filtered() {
    init_logs();

    for mut config in re_arrow_store::test_util::all_configs() {
        // NOTE: insert IDs aren't serialized and can be different across runs.
        config.store_insert_ids = false;

        let mut store1 = DataStore::new(InstanceKey::name(), config.clone());
        let mut store2 = DataStore::new(InstanceKey::name(), config.clone());

        data_store_dump_filtered_impl(&mut store1, &mut store2);

        // stress-test GC impl
        store1.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0));
        store2.gc(GarbageCollectionTarget::DropAtLeastFraction(1.0));

        data_store_dump_filtered_impl(&mut store1, &mut store2);
    }
}

fn data_store_dump_filtered_impl(store1: &mut DataStore, store2: &mut DataStore) {
    let timeline_frame_nr = Timeline::new_sequence("frame_nr");
    let timeline_log_time = Timeline::log_time();
    let frame1: TimeInt = 1.into();
    let frame2: TimeInt = 2.into();
    let frame3: TimeInt = 3.into();
    let frame4: TimeInt = 4.into();

    let ent_paths = ["this/that", "other", "yet/another/one"];
    let tables = ent_paths
        .iter()
        .map(|ent_path| create_insert_table(*ent_path))
        .collect_vec();

    // Fill the first store.
    for table in &tables {
        store1.insert_table(table).unwrap();
    }
    sanity_unwrap(store1);

    // Dump frame1 from the first store into the second one.
    for table in store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame1, frame1)).into()) {
        store2.insert_table(&table).unwrap();
    }
    // Dump frame2 from the first store into the second one.
    for table in store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame2, frame2)).into()) {
        store2.insert_table(&table).unwrap();
    }
    // Dump frame3 from the first store into the second one.
    for table in store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame3, frame3)).into()) {
        store2.insert_table(&table).unwrap();
    }
    // Dump the other frame3 from the first store into the second one.
    for table in store1.to_data_tables((timeline_log_time, TimeRange::new(frame3, frame3)).into()) {
        store2.insert_table(&table).unwrap();
    }
    // Dump frame4 from the first store into the second one.
    for table in store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame4, frame4)).into()) {
        store2.insert_table(&table).unwrap();
    }
    sanity_unwrap(store2);

    #[cfg(feature = "polars")]
    {
        let store1_df = store1.to_dataframe();
        let store2_df = store2.to_dataframe();
        assert!(
            store1_df == store2_df,
            "First & second stores differ:\n{store1_df}\n{store2_df}"
        );
    }

    let store1_stats = DataStoreStats::from_store(store1);
    let store2_stats = DataStoreStats::from_store(store2);
    assert!(
        store1_stats.temporal.num_bytes <= store2_stats.temporal.num_bytes
            && store1_stats.timeless.num_bytes <= store2_stats.timeless.num_bytes,
        "First store should have <= amount of data of second store:\n\
            {store1_stats:#?}\n{store2_stats:#?}"
    );
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

fn create_insert_table(ent_path: impl Into<EntityPath>) -> DataTable {
    let ent_path = ent_path.into();

    let frame1: TimeInt = 1.into();
    let frame2: TimeInt = 2.into();
    let frame3: TimeInt = 3.into();
    let frame4: TimeInt = 4.into();

    let (instances1, colors1) = (build_some_instances(3), build_some_colors(3));
    let row1 = test_row!(ent_path @ [
            build_frame_nr(frame1),
        ] => 3; [instances1.clone(), colors1]);

    let points2 = build_some_point2d(3);
    let row2 = test_row!(ent_path @ [
            build_frame_nr(frame2),
        ] => 3; [instances1, points2]);

    let points3 = build_some_point2d(10);
    let row3 = test_row!(ent_path @ [
            build_log_time(frame3.into()) /* ! */, build_frame_nr(frame3),
        ] => 10; [points3]);

    let colors4 = build_some_colors(5);
    let row4 = test_row!(ent_path @ [
            build_frame_nr(frame4),
        ] => 5; [colors4]);

    let mut table = DataTable::from_rows(TableId::random(), [row1, row2, row3, row4]);
    table.compute_all_size_bytes();

    table
}

// See: https://github.com/rerun-io/rerun/pull/2007
#[test]
fn data_store_dump_empty_column() {
    init_logs();

    // Split tables on 1 row
    let mut config = re_arrow_store::DataStoreConfig {
        indexed_bucket_num_rows: 1,
        ..re_arrow_store::DataStoreConfig::DEFAULT
    };
    config.store_insert_ids = false;

    let mut store = DataStore::new(InstanceKey::name(), config);

    data_store_dump_empty_column_impl(&mut store);
}

fn data_store_dump_empty_column_impl(store: &mut DataStore) {
    let ent_path: EntityPath = "points".into();
    let frame1: TimeInt = 1.into();
    let frame2: TimeInt = 2.into();
    let frame3: TimeInt = 3.into();

    // Start by inserting a table with 2 rows, one with colors, and one with points.
    {
        let (instances1, colors1) = (build_some_instances(3), build_some_colors(3));
        let row1 = test_row!(ent_path @ [
                build_frame_nr(frame1),
            ] => 3; [instances1, colors1]);

        let (instances2, points2) = (build_some_instances(3), build_some_point2d(3));
        let row2 = test_row!(ent_path @ [
            build_frame_nr(frame2),
        ] => 3; [instances2, points2]);
        let mut table = DataTable::from_rows(TableId::random(), [row1, row2]);
        table.compute_all_size_bytes();
        store.insert_table(&table).unwrap();
    }

    // Now insert another table with points only.
    {
        let (instances3, points3) = (build_some_instances(3), build_some_colors(3));
        let row3 = test_row!(ent_path @ [
                build_frame_nr(frame3),
            ] => 3; [instances3, points3]);
        let mut table = DataTable::from_rows(TableId::random(), [row3]);
        table.compute_all_size_bytes();
        store.insert_table(&table).unwrap();
    }

    let data_msgs: Result<Vec<_>, _> = store
        .to_data_tables(None)
        .map(|table| table.to_arrow_msg())
        .collect();

    // Should end up with 2 tables
    assert_eq!(data_msgs.unwrap().len(), 2);
}
