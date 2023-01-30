//! Demonstrates usage of [`re_arrow_store::polars_util::latest_component`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_arrow_store --example latest_component
//! ```

use re_arrow_store::polars_util::latest_component;
use re_arrow_store::{test_bundle, DataStore, LatestAtQuery, TimeType, Timeline};
use re_log_types::datagen::build_some_rects;
use re_log_types::field_types::Rect2D;
use re_log_types::{
    datagen::{build_frame_nr, build_some_point2d},
    field_types::{Instance, Point2D},
    msg_bundle::Component,
    EntityPath,
};

fn main() {
    let mut store = DataStore::new(Instance::name(), Default::default());

    let ent_path = EntityPath::from("my/entity");

    let bundle = test_bundle!(ent_path @ [build_frame_nr(2.into())] => [build_some_rects(4)]);
    store.insert(&bundle).unwrap();

    let bundle = test_bundle!(ent_path @ [build_frame_nr(3.into())] => [build_some_point2d(2)]);
    store.insert(&bundle).unwrap();

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
