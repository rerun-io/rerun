//! Demonstrates usage of [`re_arrow_store::polars_util::range_components`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_arrow_store --example range_components
//! ```

use polars_core::prelude::JoinType;
use re_arrow_store::{polars_util, test_bundle, DataStore, RangeQuery, TimeRange};
use re_log_types::{
    component_types::{InstanceKey, Point2D, Rect2D},
    datagen::{build_frame_nr, build_some_point2d, build_some_rects},
    msg_bundle::Component as _,
    EntityPath, TimeType, Timeline,
};

fn main() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path = EntityPath::from("this/that");

    let frame1 = 1.into();
    let frame2 = 2.into();
    let frame3 = 3.into();
    let frame4 = 4.into();

    let bundle = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [build_some_rects(2)]);
    store.insert(&bundle).unwrap();

    let bundle = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [build_some_point2d(2)]);
    store.insert(&bundle).unwrap();

    let bundle = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [build_some_point2d(4)]);
    store.insert(&bundle).unwrap();

    let bundle = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [build_some_rects(3)]);
    store.insert(&bundle).unwrap();

    let bundle = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [build_some_point2d(1)]);
    store.insert(&bundle).unwrap();

    let bundle = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [build_some_rects(3)]);
    store.insert(&bundle).unwrap();

    let bundle = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [build_some_point2d(3)]);
    store.insert(&bundle).unwrap();

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let query = RangeQuery::new(timeline_frame_nr, TimeRange::new(2.into(), 4.into()));

    println!("Store contents:\n{}", store.to_dataframe());

    println!("\n-----\n");

    let dfs = polars_util::range_components(
        &store,
        &query,
        &ent_path,
        Rect2D::name(),
        [InstanceKey::name(), Rect2D::name(), Point2D::name()],
        &JoinType::Outer,
    );
    for (time, df) in dfs.map(Result::unwrap) {
        eprintln!(
            "Found data at time {} from {}'s PoV (outer-joining):\n{}",
            time.map_or_else(
                || "<timeless>".into(),
                |time| TimeType::Sequence.format(time)
            ),
            Rect2D::name(),
            df,
        );
    }

    println!("\n-----\n");

    let dfs = polars_util::range_components(
        &store,
        &query,
        &ent_path,
        Point2D::name(),
        [InstanceKey::name(), Rect2D::name(), Point2D::name()],
        &JoinType::Outer,
    );
    for (time, df) in dfs.map(Result::unwrap) {
        eprintln!(
            "Found data at time {} from {}'s PoV (outer-joining):\n{}",
            time.map_or_else(
                || "<timeless>".into(),
                |time| TimeType::Sequence.format(time)
            ),
            Point2D::name(),
            df,
        );
    }
}
