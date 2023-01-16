//! Demonstrates usage of [`DataStore::to_dataframe`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_arrow_store --example dump_dataframe
//! ```

use re_arrow_store::{test_bundle, DataStore};
use re_log_types::{
    datagen::{
        build_frame_nr, build_log_time, build_some_instances, build_some_instances_from,
        build_some_point2d, build_some_rects,
    },
    field_types::Instance,
    msg_bundle::Component as _,
    ObjPath as EntityPath, Time,
};

// ---

fn main() {
    let mut store = DataStore::new(Instance::name(), Default::default());

    let ent_paths = [
        EntityPath::from("this/that"),
        EntityPath::from("and/this/other/thing"),
    ];

    for ent_path in &ent_paths {
        let bundle1 = test_bundle!(ent_path @ [
            build_frame_nr(1.into()), build_log_time(Time::now()),
        ] => [build_some_instances(2), build_some_rects(2)]);
        store.insert(&bundle1).unwrap();
    }

    for ent_path in &ent_paths {
        let bundle2 = test_bundle!(ent_path @ [
            build_frame_nr(2.into())
        ] => [build_some_instances(2), build_some_point2d(2)]);
        store.insert(&bundle2).unwrap();
        // Insert timelessly too!
        let bundle2 =
            test_bundle!(ent_path @ [] => [build_some_instances(2), build_some_point2d(2)]);
        store.insert(&bundle2).unwrap();

        let bundle3 = test_bundle!(ent_path @ [
            build_frame_nr(3.into()), build_log_time(Time::now()),
        ] => [build_some_instances_from(25..29), build_some_point2d(4)]);
        store.insert(&bundle3).unwrap();
        // Insert timelessly too!
        let bundle3 = test_bundle!(ent_path @ [] => [build_some_instances_from(25..29), build_some_point2d(4)]);
        store.insert(&bundle3).unwrap();
    }

    for ent_path in &ent_paths {
        let bundle4_1 = test_bundle!(ent_path @ [
            build_frame_nr(4.into()), build_log_time(Time::now()),
        ] => [build_some_instances_from(20..23), build_some_rects(3)]);
        store.insert(&bundle4_1).unwrap();

        let bundle4_15 = test_bundle!(ent_path @ [
            build_frame_nr(4.into()),
        ] => [build_some_instances_from(20..23), build_some_point2d(3)]);
        store.insert(&bundle4_15).unwrap();

        let bundle4_2 = test_bundle!(ent_path @ [
            build_frame_nr(4.into()), build_log_time(Time::now()),
        ] => [build_some_instances_from(25..28), build_some_rects(3)]);
        store.insert(&bundle4_2).unwrap();

        let bundle4_25 = test_bundle!(ent_path @ [
            build_frame_nr(4.into()), build_log_time(Time::now()),
        ] => [build_some_instances_from(25..28), build_some_point2d(3)]);
        store.insert(&bundle4_25).unwrap();
    }

    let df = store.to_dataframe();
    println!("{df}");
}
