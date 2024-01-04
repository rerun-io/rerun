//! Demonstrates usage of [`re_data_store::polars_util::latest_components`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_data_store --example latest_components
//! ```

use polars_core::prelude::*;

use re_data_store::polars_util::latest_components;
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
    let df = latest_components(
        &store,
        &LatestAtQuery::new(timeline_frame_nr, 10.into()),
        &ent_path,
        &[Position2D::name(), LargeStruct::name()],
        &JoinType::Outer,
    )
    .unwrap();

    println!("Store contents:\n{}", store.to_dataframe());
    println!("Query results:\n{df}");
}
