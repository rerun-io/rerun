mod common;

use re_arrow_store::DataStore;
use re_log_types::{build_frame_nr, DataRow, RowId};
use re_query::query_archetype;
use re_types::{
    archetypes::Points2D,
    components::{Color, InstanceKey, Position2D},
    Loggable,
};

#[test]
fn simple_query() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with implicit instances
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::random(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Assign one of them a color with an explicit instance
    let color_instances = vec![InstanceKey(1)];
    let colors = vec![Color::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::random(),
        ent_path,
        timepoint,
        1,
        (color_instances, colors),
    )
    .unwrap();
    store.insert_row(&row).unwrap();

    // Retrieve the view
    let timeline_query = re_arrow_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);

    let arch_view = query_archetype::<Points2D>(&store, &timeline_query, &ent_path.into()).unwrap();

    // We expect this to generate the following `DataFrame`
    // ┌──────────┬───────────┬────────────┐
    // │ instance ┆ positions2D┆ colorrgba  │
    // │ ---      ┆ ---       ┆ ---        │
    // │ u64      ┆ struct[2] ┆ u32        │
    // ╞══════════╪═══════════╪════════════╡
    // │ 0        ┆ {1.0,2.0} ┆ null       │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1        ┆ {3.0,4.0} ┆ 4278190080 │
    // └──────────┴───────────┴────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors = vec![None, Some(Color::from_rgb(255, 0, 0))];
        let expected = df_builder3(&instances, &positions, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        common::compare_df(&expected, &arch_view.as_df2::<Position2D, Color>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        let _used = arch_view;
    }
}

#[test]
fn timeless_query() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with implicit instances
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::random(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Assign one of them a color with an explicit instance.. timelessly!
    let color_instances = vec![InstanceKey(1)];
    let colors = vec![Color::from_rgb(255, 0, 0)];
    let row =
        DataRow::from_cells2_sized(RowId::random(), ent_path, [], 1, (color_instances, colors))
            .unwrap();
    store.insert_row(&row).unwrap();

    // Retrieve the view
    let timeline_query = re_arrow_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);

    let arch_view = query_archetype::<Points2D>(&store, &timeline_query, &ent_path.into()).unwrap();

    // We expect this to generate the following `DataFrame`
    // ┌──────────┬───────────┬────────────┐
    // │ instance ┆ Position2D┆ colorrgba  │
    // │ ---      ┆ ---       ┆ ---        │
    // │ u64      ┆ struct[2] ┆ u32        │
    // ╞══════════╪═══════════╪════════════╡
    // │ 0        ┆ {1.0,2.0} ┆ null       │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1        ┆ {3.0,4.0} ┆ 4278190080 │
    // └──────────┴───────────┴────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors = vec![None, Some(Color::from_rgb(255, 0, 0))];
        let expected = df_builder3(&instances, &positions, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        common::compare_df(&expected, &arch_view.as_df2::<Position2D, Color>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        let _used = arch_view;
    }
}

#[test]
fn no_instance_join_query() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with an implicit instance
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::random(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Assign them colors with explicit instances
    let colors = vec![Color::from_rgb(255, 0, 0), Color::from_rgb(0, 255, 0)];
    let row = DataRow::from_cells1_sized(RowId::random(), ent_path, timepoint, 2, colors).unwrap();
    store.insert_row(&row).unwrap();

    // Retrieve the view
    let timeline_query = re_arrow_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);

    let arch_view = query_archetype::<Points2D>(&store, &timeline_query, &ent_path.into()).unwrap();

    // We expect this to generate the following `DataFrame`
    // ┌──────────┬───────────┬────────────┐
    // │ instance ┆ Position2D┆ colorrgba  │
    // │ ---      ┆ ---       ┆ ---        │
    // │ u64      ┆ struct[2] ┆ u32        │
    // ╞══════════╪═══════════╪════════════╡
    // │ 0        ┆ {1.0,2.0} ┆ 4278190080 │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1        ┆ {3.0,4.0} ┆ 16711680   │
    // └──────────┴───────────┴────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors = vec![
            Some(Color::from_rgb(255, 0, 0)),
            Some(Color::from_rgb(0, 255, 0)),
        ];
        let expected = df_builder3(&instances, &positions, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        common::compare_df(&expected, &arch_view.as_df2::<Position2D, Color>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        let _used = arch_view;
    }
}

#[test]
fn missing_column_join_query() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with an implicit instance
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::random(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Retrieve the view
    let timeline_query = re_arrow_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);

    let arch_view = query_archetype::<Points2D>(&store, &timeline_query, &ent_path.into()).unwrap();

    // We expect this to generate the following `DataFrame`
    //
    // ┌──────────┬───────────┐
    // │ instance ┆ Position2D   │
    // │ ---      ┆ ---       │
    // │ u64      ┆ struct[2] │
    // ╞══════════╪═══════════╡
    // │ 0        ┆ {1.0,2.0} │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1        ┆ {3.0,4.0} │
    // └──────────┴───────────┘
    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder2;

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let expected = df_builder2(&instances, &positions).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        common::compare_df(&expected, &arch_view.as_df1::<Position2D>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        let _used = arch_view;
    }
}

#[test]
fn splatted_query() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123.into())];

    // Create some positions with implicit instances
    let positions = vec![Position2D::new(1.0, 2.0), Position2D::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::random(), ent_path, timepoint, 2, positions).unwrap();
    store.insert_row(&row).unwrap();

    // Assign all of them a color via splat
    let color_instances = vec![InstanceKey::SPLAT];
    let colors = vec![Color::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::random(),
        ent_path,
        timepoint,
        1,
        (color_instances, colors),
    )
    .unwrap();
    store.insert_row(&row).unwrap();

    // Retrieve the view
    let timeline_query = re_arrow_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);

    let arch_view = query_archetype::<Points2D>(&store, &timeline_query, &ent_path.into()).unwrap();

    // We expect this to generate the following `DataFrame`
    // ┌──────────┬───────────┬────────────┐
    // │ instance ┆ Position2D┆ colorrgba  │
    // │ ---      ┆ ---       ┆ ---        │
    // │ u64      ┆ struct[2] ┆ u32        │
    // ╞══════════╪═══════════╪════════════╡
    // │ 0        ┆ {1.0,2.0} ┆ 4278190080 │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1        ┆ {3.0,4.0} ┆ 4278190080 │
    // └──────────┴───────────┴────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let positions = vec![
            Some(Position2D::new(1.0, 2.0)),
            Some(Position2D::new(3.0, 4.0)),
        ];
        let colors = vec![
            Some(Color::from_rgb(255, 0, 0)),
            Some(Color::from_rgb(255, 0, 0)),
        ];
        let expected = df_builder3(&instances, &positions, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        common::compare_df(&expected, &arch_view.as_df2::<Position2D, Color>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        let _used = arch_view;
    }
}
