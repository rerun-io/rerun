//! Dumping a datastore to log messages and back.

use itertools::Itertools;
use re_data_store::{
    test_row,
    test_util::{insert_table_with_retries, sanity_unwrap},
    DataStoreStats, GarbageCollectionOptions, TimeInt, TimeRange, Timeline, UnaryDataStore,
};
use re_log_types::{
    build_frame_nr, build_log_time, DataRow, DataTable, EntityPath, RowId, TableId,
};
use re_types::components::InstanceKey;
use re_types::datagen::{build_some_colors, build_some_instances, build_some_positions2d};
use re_types_core::Loggable as _;

// ---

// Panic on RowId clash.
fn insert_table(store: &mut UnaryDataStore, table: &DataTable) {
    for row in table.to_rows() {
        let row = row.unwrap();
        store.insert_row(&row).unwrap();
    }
}

// ---

/// Allows adding more data to the same `RowId`.
#[derive(Default)]
struct RowSet(ahash::HashMap<RowId, DataRow>);

impl RowSet {
    fn insert_tables(&mut self, tables: impl Iterator<Item = DataTable>) {
        for table in tables {
            self.insert_table(&table);
        }
    }

    fn insert_table(&mut self, table: &DataTable) {
        for row in table.to_rows() {
            self.insert_row(row.unwrap());
        }
    }

    fn insert_row(&mut self, row: re_log_types::DataRow) {
        match self.0.entry(row.row_id()) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                assert_eq!(entry.get().entity_path(), row.entity_path());
                assert_eq!(entry.get().cells(), row.cells());
                assert_eq!(entry.get().num_instances(), row.num_instances());
                for (timeline, time) in row.timepoint() {
                    entry.get_mut().timepoint.insert(*timeline, *time);
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(row);
            }
        }
    }

    fn insert_into(self, store: &mut UnaryDataStore) {
        let mut rows = self.0.into_values().collect::<Vec<_>>();
        rows.sort_by_key(|row| (row.timepoint.clone(), row.row_id));
        for row in rows {
            store.insert_row(&row).unwrap();
        }
    }
}

// --- Dump ---

#[test]
fn data_store_dump() {
    re_log::setup_logging();

    for mut config in re_data_store::test_util::all_configs() {
        // NOTE: insert IDs aren't serialized and can be different across runs.
        config.store_insert_ids = false;

        let mut store1 = UnaryDataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        let mut store2 = UnaryDataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        let mut store3 = UnaryDataStore::new(
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

fn data_store_dump_impl(
    store1: &mut UnaryDataStore,
    store2: &mut UnaryDataStore,
    store3: &mut UnaryDataStore,
) {
    let ent_paths = ["this/that", "other", "yet/another/one"];
    let tables = ent_paths
        .iter()
        .map(|ent_path| create_insert_table(*ent_path))
        .collect_vec();

    // Fill the first store.
    for table in &tables {
        // insert temporal
        insert_table(store1, table);

        // insert timeless
        let mut table_timeless = table.clone();
        table_timeless.col_timelines = Default::default();
        insert_table_with_retries(store1, &table_timeless);
    }
    sanity_unwrap(store1);

    // Dump the first store into the second one.
    {
        // We use a RowSet instead of a DataTable to handle duplicate RowIds.
        let mut row_set = RowSet::default();
        row_set.insert_tables(store1.to_data_tables(None));
        row_set.insert_into(store2);
        sanity_unwrap(store2);
    }

    // Dump the second store into the third one.
    {
        let mut row_set = RowSet::default();
        row_set.insert_tables(store2.to_data_tables(None));
        row_set.insert_into(store3);
        sanity_unwrap(store3);
    }

    {
        let table_id = TableId::new(); // Reuse TableId so == works
        let table1 = DataTable::from_rows(table_id, store1.to_rows().unwrap());
        let table2 = DataTable::from_rows(table_id, store2.to_rows().unwrap());
        let table3 = DataTable::from_rows(table_id, store3.to_rows().unwrap());
        assert!(
            table1 == table2,
            "First & second stores differ:\n{table1}\n{table2}"
        );
        assert!(
            table1 == table3,
            "First & third stores differ:\n{table1}\n{table3}"
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
    re_log::setup_logging();

    for mut config in re_data_store::test_util::all_configs() {
        // NOTE: insert IDs aren't serialized and can be different across runs.
        config.store_insert_ids = false;

        let mut store1 = UnaryDataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            config.clone(),
        );
        let mut store2 = UnaryDataStore::new(
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

fn data_store_dump_filtered_impl(store1: &mut UnaryDataStore, store2: &mut UnaryDataStore) {
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
        insert_table(store1, table);
    }
    sanity_unwrap(store1);

    // We use a RowSet instead of a DataTable to handle duplicate RowIds.
    let mut row_set = RowSet::default();

    // Dump frame1 from the first store.
    row_set.insert_tables(
        store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame1, frame1)).into()),
    );
    // Dump frame2 from the first store.
    row_set.insert_tables(
        store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame2, frame2)).into()),
    );
    // Dump frame3 from the first store.
    row_set.insert_tables(
        store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame3, frame3)).into()),
    );
    // Dump frame3 _from the other timeline_, from the first store.
    // This will produce the same RowIds again!
    row_set.insert_tables(
        store1.to_data_tables((timeline_log_time, TimeRange::new(frame3, frame3)).into()),
    );
    // Dump frame4 from the first store.
    row_set.insert_tables(
        store1.to_data_tables((timeline_frame_nr, TimeRange::new(frame4, frame4)).into()),
    );

    row_set.insert_into(store2);
    sanity_unwrap(store2);

    {
        let table_id = TableId::new(); // Reuse TableId so == works
        let table1 = DataTable::from_rows(table_id, store1.to_rows().unwrap());
        let table2 = DataTable::from_rows(table_id, store2.to_rows().unwrap());
        assert!(
            table1 == table2,
            "First & second stores differ:\n{table1}\n{table2}"
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
    re_log::setup_logging();

    // Split tables on 1 row
    let mut config = re_data_store::DataStoreConfig {
        indexed_bucket_num_rows: 1,
        ..re_data_store::DataStoreConfig::DEFAULT
    };
    config.store_insert_ids = false;

    let mut store = UnaryDataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        config,
    );

    data_store_dump_empty_column_impl(&mut store);
}

fn data_store_dump_empty_column_impl(store: &mut UnaryDataStore) {
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
