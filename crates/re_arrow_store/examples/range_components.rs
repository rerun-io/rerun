//! Demonstrates usage of [`re_arrow_store::polars_util::range_components`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_arrow_store --example range_components
//! ```

use polars_core::prelude::JoinType;
use re_arrow_store::{polars_util, test_row, DataStore, RangeQuery, TimeRange};
use re_components::datagen::{build_frame_nr, build_some_point2d};
use re_log_types::{EntityPath, TimeType, Timeline};
use re_types::{
    components::{InstanceKey, Point2D},
    testing::{build_some_large_structs, LargeStruct},
    Loggable as _,
};

fn main() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path = EntityPath::from("this/that");

    let frame1 = 1.into();
    let frame2 = 2.into();
    let frame3 = 3.into();
    let frame4 = 4.into();

    let row = test_row!(ent_path @ [build_frame_nr(frame1)] => 2; [build_some_large_structs(2)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame2)] => 2; [build_some_point2d(2)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame3)] => 4; [build_some_point2d(4)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame4)] => 3; [build_some_large_structs(3)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame4)] => 1; [build_some_point2d(1)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame4)] => 3; [build_some_large_structs(3)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame4)] => 3; [build_some_point2d(3)]);
    store.insert_row(&row).unwrap();

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let query = RangeQuery::new(timeline_frame_nr, TimeRange::new(2.into(), 4.into()));

    println!("Store contents:\n{}", store.to_dataframe());

    println!("\n-----\n");

    let dfs = polars_util::range_components(
        &store,
        &query,
        &ent_path,
        LargeStruct::name(),
        [InstanceKey::name(), LargeStruct::name(), Point2D::name()],
        &JoinType::Outer,
    );
    for (time, df) in dfs.map(Result::unwrap) {
        eprintln!(
            "Found data at time {} from {}'s PoV (outer-joining):\n{}",
            time.map_or_else(
                || "<timeless>".into(),
                |time| TimeType::Sequence.format(time)
            ),
            LargeStruct::name(),
            df,
        );
    }

    println!("\n-----\n");

    let dfs = polars_util::range_components(
        &store,
        &query,
        &ent_path,
        Point2D::name(),
        [InstanceKey::name(), LargeStruct::name(), Point2D::name()],
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
