//! Demonstrates usage of [`re_query::range_entity_with_primary`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_query --all-features --example range
//! ```

use re_arrow_store::{DataStore, RangeQuery, TimeRange};
use re_log_types::{
    component_types::{InstanceKey, Point2D, Rect2D},
    datagen::{build_frame_nr, build_some_point2d, build_some_rects},
    Component as _, DataRow, EntityPath, RowId, TimeType,
};
use re_query::range_entity_with_primary;

fn main() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path: EntityPath = "point".into();

    let frame1 = [build_frame_nr(1.into())];
    let frame2 = [build_frame_nr(2.into())];
    let frame3 = [build_frame_nr(3.into())];
    let frame4 = [build_frame_nr(4.into())];

    let rects = build_some_rects(2);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame1, 2, &rects);
    store.insert_row(&row).unwrap();

    let points = build_some_point2d(2);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame2, 2, &points);
    store.insert_row(&row).unwrap();

    let points = build_some_point2d(4);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame3, 4, &points);
    store.insert_row(&row).unwrap();

    let rects = build_some_rects(3);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame4, 3, &rects);
    store.insert_row(&row).unwrap();

    let points = build_some_point2d(3);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame4, 3, &points);
    store.insert_row(&row).unwrap();

    let rects = build_some_rects(3);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame4, 3, &rects);
    store.insert_row(&row).unwrap();

    let query = RangeQuery::new(frame2[0].0, TimeRange::new(frame2[0].1, frame4[0].1));

    println!("Store contents:\n{}", store.to_dataframe());

    println!("\n-----\n");

    let components = [InstanceKey::name(), Rect2D::name(), Point2D::name()];
    let ent_views = range_entity_with_primary::<Rect2D, 3>(&store, &query, &ent_path, components);
    for (time, ent_view) in ent_views {
        eprintln!(
            "Found data at time {} from {}'s PoV:\n{}",
            time.map_or_else(
                || "<timeless>".into(),
                |time| TimeType::Sequence.format(time)
            ),
            Rect2D::name(),
            &ent_view.as_df2::<Point2D>().unwrap()
        );
    }

    println!("\n-----\n");

    let components = [InstanceKey::name(), Rect2D::name(), Point2D::name()];
    let ent_views = range_entity_with_primary::<Point2D, 3>(&store, &query, &ent_path, components);
    for (time, ent_view) in ent_views {
        eprintln!(
            "Found data at time {} from {}'s PoV:\n{}",
            time.map_or_else(
                || "<timeless>".into(),
                |time| TimeType::Sequence.format(time)
            ),
            Point2D::name(),
            &ent_view.as_df2::<Rect2D>().unwrap()
        );
    }
}
