//! Demonstrates usage of [`re_data_store::api::latest_component`].
//!
//! ```text
//! cargo r -p re_data_store --example latest_component
//! ```

use re_data_store::api::latest_component;
use re_data_store::{test_row, DataStore, LatestAtQuery, TimeType, Timeline};
use re_log_types::{build_frame_nr, EntityPath};
use re_types::datagen::build_some_positions2d;
use re_types::{
    components::{InstanceKey, Position2D},
    testing::{build_some_large_structs, LargeStruct},
};
use re_types_core::Loggable as _;

fn main() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let ent_path = EntityPath::from("my/entity");

    let row = test_row!(ent_path @ [build_frame_nr(2.into())] => 4; [build_some_large_structs(4)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(3.into())] => 2; [build_some_positions2d(2)]);
    store.insert_row(&row).unwrap();

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

    println!("Store contents:\n{}", store.to_data_table().unwrap());

    println!("\n-----\n");

    let table = latest_component(
        &store,
        &LatestAtQuery::new(timeline_frame_nr, 10.into()),
        &ent_path,
        LargeStruct::name(),
    )
    .unwrap();
    println!(
        "Query results from {:?}'s PoV:\n{table}",
        LargeStruct::name()
    );

    println!("\n-----\n");

    let table = latest_component(
        &store,
        &LatestAtQuery::new(timeline_frame_nr, 10.into()),
        &ent_path,
        Position2D::name(),
    )
    .unwrap();
    println!(
        "Query results from {:?}'s PoV:\n{table}",
        Position2D::name()
    );
}
