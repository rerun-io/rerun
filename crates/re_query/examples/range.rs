//! Demonstrates usage of [`re_query::range_entity_with_primary`].
//!
//! ```text
//! POLARS_FMT_MAX_ROWS=100 cargo r -p re_query --all-features --example range
//! ```

use re_arrow_store::{DataStore, RangeQuery, TimeRange};
use re_components::datagen::{build_frame_nr, build_some_colors, build_some_positions2d};
use re_log_types::{DataRow, EntityPath, RowId, TimeType};
use re_query::range_entity_with_primary;
use re_types::{
    components::{Color, InstanceKey, Position2D},
    Loggable as _,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path: EntityPath = "point".into();

    let frame1 = [build_frame_nr(1.into())];
    let frame2 = [build_frame_nr(2.into())];
    let frame3 = [build_frame_nr(3.into())];
    let frame4 = [build_frame_nr(4.into())];

    let colors = build_some_colors(2);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame1, 2, &colors)?;
    store.insert_row(&row)?;

    let positions = build_some_positions2d(2);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame2, 2, &positions)?;
    store.insert_row(&row)?;

    let positions = build_some_positions2d(4);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame3, 4, &positions)?;
    store.insert_row(&row)?;

    let colors = build_some_colors(3);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame4, 3, &colors)?;
    store.insert_row(&row)?;

    let positions = build_some_positions2d(3);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame4, 3, &positions)?;
    store.insert_row(&row)?;

    let colors = build_some_colors(3);
    let row = DataRow::from_cells1(RowId::random(), ent_path.clone(), frame4, 3, &colors)?;
    store.insert_row(&row)?;

    let query = RangeQuery::new(frame2[0].0, TimeRange::new(frame2[0].1, frame4[0].1));

    println!("Store contents:\n{}", store.to_dataframe());

    // TODO(andreas): range_entity_with_primary only works with legacy components.
    // println!("\n-----\n");
    // let components = [InstanceKey::name(), Color::name(), Point2D::name()];
    // let ent_views = range_entity_with_primary::<Color, 3>(&store, &query, &ent_path, components);
    // for (time, ent_view) in ent_views {
    //     eprintln!(
    //         "Found data at time {} from {}'s PoV:\n{}",
    //         time.map_or_else(
    //             || "<timeless>".into(),
    //             |time| TimeType::Sequence.format(time)
    //         ),
    //         Color::name(),
    //         &ent_view.as_df2::<Point2D>()?
    //     );
    // }

    println!("\n-----\n");

    let components = [InstanceKey::name(), Color::name(), Position2D::name()];
    let ent_views =
        range_entity_with_primary::<Position2D, 3>(&store, &query, &ent_path, components);
    for (time, ent_view) in ent_views {
        eprintln!(
            "Found data at time {} from {}'s PoV:\n{}",
            time.map_or_else(
                || "<timeless>".into(),
                |time| TimeType::Sequence.format(time)
            ),
            Position2D::name(),
            &ent_view.as_df2::<Color>()?
        );
    }

    Ok(())
}
