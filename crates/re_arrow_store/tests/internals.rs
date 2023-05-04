//! Tests running assertions on the internal state of the datastore.
//!
//! They're awful, but sometimes you just have to...

use std::sync::atomic::{AtomicBool, Ordering::SeqCst};

use re_arrow_store::{DataStore, DataStoreConfig};
use re_log_types::{
    component_types::InstanceKey,
    datagen::{build_frame_nr, build_some_instances},
    Component as _, DataRow, EntityPath, RowId, TimePoint,
};

// --- Internals ---

// TODO(cmc): One should _never_ run assertions on the internal state of the datastore, this
// is a recipe for disaster.
//
// The contract that needs to be asserted here, from the point of view of the actual user,
// is performance: getting the datastore into a pathological topology should show up in
// integration query benchmarks.
//
// In the current state of things, though, it is much easier to test for it that way... so we
// make an exception, for now...
#[test]
fn pathological_bucket_topology() {
    init_logs();

    let mut store_forward = DataStore::new(
        InstanceKey::name(),
        DataStoreConfig {
            indexed_bucket_num_rows: 10,
            ..Default::default()
        },
    );
    let mut store_backward = DataStore::new(
        InstanceKey::name(),
        DataStoreConfig {
            indexed_bucket_num_rows: 10,
            ..Default::default()
        },
    );

    fn store_repeated_frame(
        frame_nr: i64,
        num: usize,
        store_forward: &mut DataStore,
        store_backward: &mut DataStore,
    ) {
        let ent_path = EntityPath::from("this/that");
        let num_instances = 1;

        let timepoint = TimePoint::from([build_frame_nr(frame_nr.into())]);
        for _ in 0..num {
            let row = DataRow::from_cells1_sized(
                RowId::random(),
                ent_path.clone(),
                timepoint.clone(),
                num_instances,
                build_some_instances(num_instances as _),
            );
            store_forward.insert_row(&row).unwrap();

            let row = DataRow::from_cells1_sized(
                RowId::random(),
                ent_path.clone(),
                timepoint.clone(),
                num_instances,
                build_some_instances(num_instances as _),
            );
            store_backward.insert_row(&row).unwrap();
        }
    }

    fn store_frame_range(
        range: core::ops::RangeInclusive<i64>,
        store_forward: &mut DataStore,
        store_backward: &mut DataStore,
    ) {
        let ent_path = EntityPath::from("this/that");
        let num_instances = 1;

        let rows = range
            .map(|frame_nr| {
                let timepoint = TimePoint::from([build_frame_nr(frame_nr.into())]);
                DataRow::from_cells1_sized(
                    RowId::random(),
                    ent_path.clone(),
                    timepoint,
                    num_instances,
                    build_some_instances(num_instances as _),
                )
            })
            .collect::<Vec<_>>();

        rows.iter()
            .for_each(|row| store_forward.insert_row(row).unwrap());

        rows.iter()
            .rev()
            .for_each(|row| store_backward.insert_row(row).unwrap());
    }

    store_repeated_frame(1000, 10, &mut store_forward, &mut store_backward);
    store_frame_range(970..=979, &mut store_forward, &mut store_backward);
    store_frame_range(990..=999, &mut store_forward, &mut store_backward);
    store_frame_range(980..=989, &mut store_forward, &mut store_backward);
    store_repeated_frame(1000, 7, &mut store_forward, &mut store_backward);
    store_frame_range(1000..=1009, &mut store_forward, &mut store_backward);
    store_repeated_frame(975, 10, &mut store_forward, &mut store_backward);

    {
        let num_buckets = store_forward
            .iter_indices()
            .flat_map(|(_, table)| table.buckets.values())
            .count();
        assert_eq!(
            7usize,
            num_buckets,
            "pathological topology (forward): {}",
            {
                store_forward.sort_indices_if_needed();
                store_forward
            }
        );
    }
    {
        let num_buckets = store_backward
            .iter_indices()
            .flat_map(|(_, table)| table.buckets.values())
            .count();
        assert_eq!(
            8usize,
            num_buckets,
            "pathological topology (backward): {}",
            {
                store_backward.sort_indices_if_needed();
                store_backward
            }
        );
    }
}

fn init_logs() {
    static INIT: AtomicBool = AtomicBool::new(false);

    if INIT.compare_exchange(false, true, SeqCst, SeqCst).is_ok() {
        re_log::setup_native_logging();
    }
}
