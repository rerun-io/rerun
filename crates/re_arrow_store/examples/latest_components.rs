//! Demonstrates usage of [`re_arrow_store::polars_util::latest_components`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_arrow_store --example latest_components
//! ```

use polars_core::prelude::*;
use re_arrow_store::polars_util::latest_components;
use re_arrow_store::{test_bundle, DataStore, LatestAtQuery, TimeType, Timeline};
use re_log_types::{
    datagen::{build_frame_nr, build_some_point2d, build_some_rects},
    field_types::{Instance, Point2D, Rect2D},
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
    let df = latest_components(
        &store,
        &LatestAtQuery::new(timeline_frame_nr, 10.into()),
        &ent_path,
        &[Point2D::name(), Rect2D::name()],
        &JoinType::Outer,
    )
    .unwrap();

    println!("Store contents:\n{}", store.to_dataframe());
    println!("Query results:\n{df}");
}
