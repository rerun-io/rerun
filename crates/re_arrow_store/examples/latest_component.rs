//! Demonstrates usage of [`re_arrow_store::polars_util::latest_component`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_arrow_store --example latest_component
//! ```

use re_arrow_store::polars_util::latest_component;
use re_arrow_store::{test_row, DataStore, LatestAtQuery, TimeType, Timeline};
use re_components::{
    datagen::{build_frame_nr, build_some_point2d, build_some_rects},
    Rect2D,
};
use re_log_types::EntityPath;
use re_types::{
    components::{InstanceKey, Point2D},
    Loggable,
};

fn main() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path = EntityPath::from("my/entity");

    let row = test_row!(ent_path @ [build_frame_nr(2.into())] => 4; [build_some_rects(4)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(3.into())] => 2; [build_some_point2d(2)]);
    store.insert_row(&row).unwrap();

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

    println!("Store contents:\n{}", store.to_dataframe());

    println!("\n-----\n");

    let df = latest_component(
        &store,
        &LatestAtQuery::new(timeline_frame_nr, 10.into()),
        &ent_path,
        Rect2D::name(),
    )
    .unwrap();
    println!("Query results from {:?}'s PoV:\n{df}", Rect2D::name());

    println!("\n-----\n");

    let df = latest_component(
        &store,
        &LatestAtQuery::new(timeline_frame_nr, 10.into()),
        &ent_path,
        Point2D::name(),
    )
    .unwrap();
    println!("Query results from {:?}'s PoV:\n{df}", Point2D::name());
}
