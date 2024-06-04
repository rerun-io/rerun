//! Dumping a datastore to log messages and back.

// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]

use itertools::Itertools;
use re_data_store2::{
    test_row,
    test_util::{insert_table_with_retries, sanity_unwrap},
    DataStore, DataStoreStats, GarbageCollectionOptions, ResolvedTimeRange, TimeInt, Timeline,
};
use re_log_types::{
    build_frame_nr, build_log_time,
    example_components::{MyColor, MyIndex, MyPoint},
    DataRow, DataTable, EntityPath, RowId, TableId, TimePoint,
};

// ---

// Panic on RowId clash.
fn insert_table(store: &mut DataStore, table: &DataTable) {
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
                for (timeline, time) in row.timepoint() {
                    entry.get_mut().timepoint.insert(*timeline, *time);
                }
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(row);
            }
        }
    }

    fn insert_into(self, store: &mut DataStore) {
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

        let mut store1 = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            config.clone(),
        );
        let mut store2 = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            config.clone(),
        );
        let mut store3 = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
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
    let entity_paths = ["this/that", "other", "yet/another/one"];
    let tables = entity_paths
        .iter()
        .map(|entity_path| create_insert_table(*entity_path))
        .collect_vec();

    // Fill the first store.
    for table in &tables {
        // insert temporal
        insert_table(store1, table);

        // insert static
        let mut table_static = table.clone();
        table_static.col_timelines = Default::default();
        insert_table_with_retries(store1, &table_static);
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
            && store1_stats.static_tables.num_bytes <= store2_stats.static_tables.num_bytes,
        "First store should have <= amount of data of second store:\n\
            {store1_stats:#?}\n{store2_stats:#?}"
    );
    assert!(
        store2_stats.temporal.num_bytes <= store3_stats.temporal.num_bytes
            && store2_stats.static_tables.num_bytes <= store3_stats.static_tables.num_bytes,
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

        let mut store1 = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            config.clone(),
        );
        let mut store2 = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
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
    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);

    let entity_paths = ["this/that", "other", "yet/another/one"];
    let tables = entity_paths
        .iter()
        .map(|entity_path| create_insert_table(*entity_path))
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
        store1.to_data_tables((timeline_frame_nr, ResolvedTimeRange::new(frame1, frame1)).into()),
    );
    // Dump frame2 from the first store.
    row_set.insert_tables(
        store1.to_data_tables((timeline_frame_nr, ResolvedTimeRange::new(frame2, frame2)).into()),
    );
    // Dump frame3 from the first store.
    row_set.insert_tables(
        store1.to_data_tables((timeline_frame_nr, ResolvedTimeRange::new(frame3, frame3)).into()),
    );
    // Dump frame3 _from the other timeline_, from the first store.
    // This will produce the same RowIds again!
    row_set.insert_tables(
        store1.to_data_tables((timeline_log_time, ResolvedTimeRange::new(frame3, frame3)).into()),
    );
    // Dump frame4 from the first store.
    row_set.insert_tables(
        store1.to_data_tables((timeline_frame_nr, ResolvedTimeRange::new(frame4, frame4)).into()),
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
            && store1_stats.static_tables.num_bytes <= store2_stats.static_tables.num_bytes,
        "First store should have <= amount of data of second store:\n\
            {store1_stats:#?}\n{store2_stats:#?}"
    );
}

// ---

fn create_insert_table(entity_path: impl Into<EntityPath>) -> DataTable {
    let entity_path = entity_path.into();

    let timeless = TimePoint::default();
    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);

    let (instances1, colors1) = (MyIndex::from_iter(0..3), MyColor::from_iter(0..3));
    let row1 = test_row!(entity_path @ [
            build_frame_nr(frame1),
        ] => [instances1.clone(), colors1]);

    let positions2 = MyPoint::from_iter(0..2);
    let row2 = test_row!(entity_path @ [
            build_frame_nr(frame2),
        ] => [instances1, positions2.clone()]);

    let positions3 = MyPoint::from_iter(0..10);
    let row3 = test_row!(entity_path @ [
            build_log_time(frame3.into()) /* ! */, build_frame_nr(frame3),
        ] => [positions3]);

    let colors4 = MyColor::from_iter(0..5);
    let row4 = test_row!(entity_path @ [
            build_frame_nr(frame4),
        ] => [colors4.clone()]);

    let row0 = test_row!(entity_path @ timeless => [positions2, colors4]);

    let mut table = DataTable::from_rows(TableId::new(), [row0, row1, row2, row3, row4]);
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

    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        config,
    );

    data_store_dump_empty_column_impl(&mut store);
}

fn data_store_dump_empty_column_impl(store: &mut DataStore) {
    let entity_path: EntityPath = "points".into();
    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);

    // Start by inserting a table with 2 rows, one with colors, and one with points.
    {
        let (instances1, colors1) = (MyIndex::from_iter(0..3), MyColor::from_iter(0..3));
        let row1 = test_row!(entity_path @ [
                build_frame_nr(frame1),
            ] => [instances1, colors1]);

        let (instances2, positions2) = (MyIndex::from_iter(0..3), MyPoint::from_iter(0..2));
        let row2 = test_row!(entity_path @ [
            build_frame_nr(frame2),
        ] => [instances2, positions2]);
        let mut table = DataTable::from_rows(TableId::new(), [row1, row2]);
        table.compute_all_size_bytes();
        insert_table_with_retries(store, &table);
    }

    // Now insert another table with points only.
    {
        let (instances3, positions3) = (MyIndex::from_iter(0..3), MyColor::from_iter(0..3));
        let row3 = test_row!(entity_path @ [
                build_frame_nr(frame3),
            ] => [instances3, positions3]);
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
