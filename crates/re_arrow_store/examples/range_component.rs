//! Demonstrates usage of [`re_arrow_store::polars_util::range_component`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_arrow_store --example range_component
//! ```

use re_arrow_store::{polars_util, test_bundle, DataStore, RangeQuery, TimeRange};
use re_log_types::{
    datagen::{
        build_frame_nr, build_some_instances, build_some_instances_from, build_some_point2d,
        build_some_rects,
    },
    field_types::{Instance, Point2D, Rect2D},
    msg_bundle::Component as _,
    ObjPath as EntityPath, TimeType, Timeline,
};

fn main() {
    let mut store = DataStore::new(Instance::name(), Default::default());

    let ent_path = EntityPath::from("this/that");

    let frame1 = 1.into();
    let frame2 = 2.into();
    let frame3 = 3.into();
    let frame4 = 4.into();

    let insts1 = build_some_instances(2);
    let rects1 = build_some_rects(2);
    let bundle1 = test_bundle!(ent_path @ [build_frame_nr(frame1)] => [insts1.clone(), rects1]);
    store.insert(&bundle1).unwrap();

    let points2 = build_some_point2d(2);
    let bundle2 = test_bundle!(ent_path @ [build_frame_nr(frame2)] => [insts1, points2]);
    store.insert(&bundle2).unwrap();

    let insts3 = build_some_instances_from(25..29);
    let points3 = build_some_point2d(4);
    let bundle3 = test_bundle!(ent_path @ [build_frame_nr(frame3)] => [insts3, points3]);
    store.insert(&bundle3).unwrap();

    let insts4_1 = build_some_instances_from(20..23);
    let rects4_1 = build_some_rects(3);
    let bundle4_1 =
        test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1.clone(), rects4_1]);
    store.insert(&bundle4_1).unwrap();

    let points4_15 = build_some_point2d(3);
    let bundle4_15 =
        test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1.clone(), points4_15]);
    store.insert(&bundle4_15).unwrap();

    let insts4_2 = build_some_instances_from(25..28);
    let rects4_2 = build_some_rects(3);
    let bundle4_2 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_2, rects4_2]);
    store.insert(&bundle4_2).unwrap();

    let points4_25 = build_some_point2d(3);
    let bundle4_25 = test_bundle!(ent_path @ [build_frame_nr(frame4)] => [insts4_1, points4_25]);
    store.insert(&bundle4_25).unwrap();

    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let query = RangeQuery {
        timeline: timeline_frame_nr,
        range: TimeRange::new(2.into(), 4.into()),
    };

    println!("Store contents:\n{}", store.to_dataframe());

    println!("\n-----\n");

    let dfs = polars_util::range_component(&store, &query, &ent_path, Rect2D::name());
    for (time, df) in dfs.map(Result::unwrap) {
        eprintln!(
            "Found data at time {} from {}'s PoV (outer-joining):\n{}",
            TimeType::Sequence.format(time),
            Rect2D::name(),
            df,
        );
    }

    println!("\n-----\n");

    let dfs = polars_util::range_component(&store, &query, &ent_path, Point2D::name());
    for (time, df) in dfs.map(Result::unwrap) {
        eprintln!(
            "Found data at time {} from {}'s PoV (outer-joining):\n{}",
            TimeType::Sequence.format(time),
            Point2D::name(),
            df,
        );
    }
}
