//! Demonstrates usage of [`re_data_store::polars_util::range_components`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_data_store --all-features --example range_components
//! ```

use polars_core::prelude::JoinType;
use re_data_store::{polars_util, test_row, DataStore, RangeQuery, TimeRange};
use re_log_types::{build_frame_nr, EntityPath, TimeType, Timeline};
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

    let ent_path = EntityPath::from("this/that");

    let frame1 = 1.into();
    let frame2 = 2.into();
    let frame3 = 3.into();
    let frame4 = 4.into();

    let row = test_row!(ent_path @ [build_frame_nr(frame1)] => 2; [build_some_large_structs(2)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame2)] => 2; [build_some_positions2d(2)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame3)] => 4; [build_some_positions2d(4)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame4)] => 3; [build_some_large_structs(3)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame4)] => 1; [build_some_positions2d(1)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame4)] => 3; [build_some_large_structs(3)]);
    store.insert_row(&row).unwrap();

    let row = test_row!(ent_path @ [build_frame_nr(frame4)] => 3; [build_some_positions2d(3)]);
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
        [InstanceKey::name(), LargeStruct::name(), Position2D::name()],
        &JoinType::Outer,
    );
    for (time, df) in dfs.map(Result::unwrap) {
        eprintln!(
            "Found data at time {} from {}'s PoV (outer-joining):\n{}",
            time.map_or_else(
                || "<timeless>".into(),
                |time| TimeType::Sequence.format_utc(time)
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
        Position2D::name(),
        [InstanceKey::name(), LargeStruct::name(), Position2D::name()],
        &JoinType::Outer,
    );
    for (time, df) in dfs.map(Result::unwrap) {
        eprintln!(
            "Found data at time {} from {}'s PoV (outer-joining):\n{}",
            time.map_or_else(
                || "<timeless>".into(),
                |time| TimeType::Sequence.format_utc(time)
            ),
            Position2D::name(),
            df,
        );
    }
}
