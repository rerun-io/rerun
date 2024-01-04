//! Dumping a datastore to log messages and back.

use std::sync::atomic::{AtomicBool, Ordering};

use itertools::Itertools;
use re_data_store::WriteError;
use re_data_store::{
    test_row, test_util::sanity_unwrap, DataStore, DataStoreStats, GarbageCollectionOptions,
    TimeInt, TimeRange, Timeline,
};
use re_log_types::{build_frame_nr, build_log_time, DataTable, EntityPath, TableId};
use re_types::components::InstanceKey;
use re_types::datagen::{build_some_colors, build_some_instances, build_some_positions2d};
use re_types_core::Loggable as _;

// ---

// We very often re-use RowIds when generating test data.
fn insert_table_with_retries(store: &mut DataStore, table: &DataTable) {
    for row in table.to_rows() {
        let mut row = row.unwrap();
        loop {
            match store.insert_row(&row) {
                Ok(_) => break,
                Err(WriteError::ReusedRowId(_)) => {
                    row.row_id = row.row_id.next();
                }
                err @ Err(_) => err.map(|_| ()).unwrap(),
            }
        }
    }
}

// --- Dump ---

#[test]
fn data_store_dump() {
    init_logs();

    for mut config in re_data_store::test_util::all_configs() {
        // NOTE: insert IDs aren't serialized and can be different across runs.
        config.store_insert_ids = false;

        let mut store1 = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        let mut store2 = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        let mut store3 = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );

        data_store_dump_impl(&mut store1, &mut store2, &mut store3);

        // stress-test GC impl
        store1.gc(&GarbageCollectionOptions::gc_everything());
        store2.gc(&GarbageCollectionOptions::gc_everything());
        store3.gc(&GarbageCollectionOptions::gc_everything());

        data_store_dump_impl(&mut store1, &mut store2, &mut store3);
    }
}

fn data_store_dump_impl(store1: &mut DataStore, store2: &mut DataStore, store3: &mut DataStore) {
    // helper to insert a table both as a temporal and timeless payload
    let insert_table = |store: &mut DataStore, table: &DataTable| {
        // insert temporal
        insert_table_with_retries(store, table);

        // insert timeless
        let mut table_timeless = table.clone();
        table_timeless.col_timelines = Default::default();
        insert_table_with_retries(store, &table_timeless);
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
        insert_table_with_retries(store2, &table);
    }
    sanity_unwrap(store2);

    // Dump the second store into the third one.
    for table in store2.to_data_tables(None) {
        insert_table_with_retries(store3, &table);
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

    for mut config in re_data_store::test_util::all_configs() {
        // NOTE: insert IDs aren't serialized and can be different across runs.
        config.store_insert_ids = false;

        let mut store1 = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        let mut store2 = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );

        data_store_dump_filtered_impl(&mut store1, &mut store2);

        // stress-test GC impl
        store1.gc(&GarbageCollectionOptions::gc_everything());
        store2.gc(&GarbageCollectionOptions::gc_everything());

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
        insert_table_with_retries(store1, table);
    }
    sanity_unwrap(store1);

    // Dump frame1 from the first store into the second one.
    for table in store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame1, frame1)).into()) {
        insert_table_with_retries(store2, &table);
    }
    // Dump frame2 from the first store into the second one.
    for table in store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame2, frame2)).into()) {
        insert_table_with_retries(store2, &table);
    }
    // Dump frame3 from the first store into the second one.
    for table in store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame3, frame3)).into()) {
        insert_table_with_retries(store2, &table);
    }
    // Dump the other frame3 from the first store into the second one.
    for table in store1.to_data_tables((timeline_log_time, TimeRange::new(frame3, frame3)).into()) {
        insert_table_with_retries(store2, &table);
    }
    // Dump frame4 from the first store into the second one.
    for table in store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame4, frame4)).into()) {
        insert_table_with_retries(store2, &table);
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

    let positions2 = build_some_positions2d(3);
    let row2 = test_row!(ent_path @ [
            build_frame_nr(frame2),
        ] => 3; [instances1, positions2]);

    let positions3 = build_some_positions2d(10);
    let row3 = test_row!(ent_path @ [
            build_log_time(frame3.into()) /* ! */, build_frame_nr(frame3),
        ] => 10; [positions3]);

    let colors4 = build_some_colors(5);
    let row4 = test_row!(ent_path @ [
            build_frame_nr(frame4),
        ] => 5; [colors4]);

    let mut table = DataTable::from_rows(TableId::new(), [row1, row2, row3, row4]);
    table.compute_all_size_bytes();

    table
}

// See: https://github.com/rerun-io/rerun/pull/2007
#[test]
fn data_store_dump_empty_column() {
    init_logs();

    // Split tables on 1 row
    let mut config = re_data_store::DataStoreConfig {
        indexed_bucket_num_rows: 1,
        ..re_data_store::DataStoreConfig::DEFAULT
    };
    config.store_insert_ids = false;

    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        config,
    );

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

        let (instances2, positions2) = (build_some_instances(3), build_some_positions2d(3));
        let row2 = test_row!(ent_path @ [
            build_frame_nr(frame2),
        ] => 3; [instances2, positions2]);
        let mut table = DataTable::from_rows(TableId::new(), [row1, row2]);
        table.compute_all_size_bytes();
        insert_table_with_retries(store, &table);
    }

    // Now insert another table with points only.
    {
        let (instances3, positions3) = (build_some_instances(3), build_some_colors(3));
        let row3 = test_row!(ent_path @ [
                build_frame_nr(frame3),
            ] => 3; [instances3, positions3]);
        let mut table = DataTable::from_rows(TableId::new(), [row3]);
        table.compute_all_size_bytes();
        insert_table_with_retries(store, &table);
    }

    let data_msgs: Result<Vec<_>, _> = store
        .to_data_tables(None)
        .map(|table| table.to_arrow_msg())
        .collect();

    // Should end up with 2 tables
    assert_eq!(data_msgs.unwrap().len(), 2);
}
