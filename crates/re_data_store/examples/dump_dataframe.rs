//! Demonstrates usage of [`DataStore::to_dataframe`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_data_store --example dump_dataframe
//! ```

use re_data_store::{test_row, DataStore};
use re_log_types::{build_frame_nr, build_log_time, EntityPath, Time};
use re_types::datagen::{build_some_instances, build_some_instances_from, build_some_positions2d};
use re_types::{components::InstanceKey, testing::build_some_large_structs};
use re_types_core::Loggable as _;

// ---

fn main() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_paths = [
        EntityPath::from("this/that"),
        EntityPath::from("and/this/other/thing"),
    ];

    for ent_path in &ent_paths {
        let row1 = test_row!(ent_path @ [
                build_frame_nr(1.into()), build_log_time(Time::now()),
            ] => 2; [build_some_instances(2), build_some_large_structs(2)]);
        store.insert_row(&row1).unwrap();
    }

    for ent_path in &ent_paths {
        let row2 = test_row!(ent_path @ [
                build_frame_nr(2.into())
            ] => 2; [build_some_instances(2), build_some_positions2d(2)]);
        store.insert_row(&row2).unwrap();
        // Insert timelessly too!
        let row2 =
            test_row!(ent_path @ [] => 2; [build_some_instances(2), build_some_positions2d(2)]);
        store.insert_row(&row2).unwrap();

        let row3 = test_row!(ent_path @ [
                build_frame_nr(3.into()), build_log_time(Time::now()),
            ] => 4; [build_some_instances_from(25..29), build_some_positions2d(4)]);
        store.insert_row(&row3).unwrap();
        // Insert timelessly too!
        let row3 = test_row!(ent_path @ [] => 4; [build_some_instances_from(25..29), build_some_positions2d(4)]);
        store.insert_row(&row3).unwrap();
    }

    for ent_path in &ent_paths {
        let row4_1 = test_row!(ent_path @ [
                build_frame_nr(4.into()), build_log_time(Time::now()),
            ] => 3; [build_some_instances_from(20..23), build_some_large_structs(3)]);
        store.insert_row(&row4_1).unwrap();

        let row4_15 = test_row!(ent_path @ [
                build_frame_nr(4.into()),
            ] => 3; [build_some_instances_from(20..23), build_some_positions2d(3)]);
        store.insert_row(&row4_15).unwrap();

        let row4_2 = test_row!(ent_path @ [
                build_frame_nr(4.into()), build_log_time(Time::now()),
            ] => 3; [build_some_instances_from(25..28), build_some_large_structs(3)]);
        store.insert_row(&row4_2).unwrap();

        let row4_25 = test_row!(ent_path @ [
                build_frame_nr(4.into()), build_log_time(Time::now()),
            ] => 3; [build_some_instances_from(25..28), build_some_positions2d(3)]);
        store.insert_row(&row4_25).unwrap();
    }

    let df = store.to_dataframe();
    println!("{df}");
}
