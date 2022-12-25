mod common;

use re_arrow_store::{DataStore, TimeQuery};
use re_log_types::{
    datagen::build_frame_nr,
    field_types::Instance,
    field_types::{ColorRGBA, Point2D},
    msg_bundle::try_build_msg_bundle1,
    msg_bundle::try_build_msg_bundle2,
    msg_bundle::Component,
    MsgId,
};
use re_query::query_entity_with_primary;

#[test]
fn simple_query() {
    let mut store = DataStore::new(Instance::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some points with implicit instances
    let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
    let bundle = try_build_msg_bundle1(MsgId::random(), ent_path, timepoint, &points).unwrap();
    store.insert(&bundle).unwrap();

    // Assign one of them a color with an explicit instance
    let color_instances = vec![Instance(1)];
    let colors = vec![ColorRGBA(0xff000000)];
    let bundle = try_build_msg_bundle2(
        MsgId::random(),
        ent_path,
        timepoint,
        (color_instances, colors),
    )
    .unwrap();
    store.insert(&bundle).unwrap();

    // Retrieve the view
    let timeline_query = re_arrow_store::TimelineQuery::new(
        timepoint[0].0,
        TimeQuery::LatestAt(timepoint[0].1.as_i64()),
    );

    let entity_view = query_entity_with_primary(
        &store,
        &timeline_query,
        &ent_path.into(),
        Point2D::name(),
        &[ColorRGBA::name()],
    )
    .unwrap();

    // We expect this to generate the following `DataFrame`
    // ┌──────────┬───────────┬────────────┐
    // │ instance ┆ point2d   ┆ colorrgba  │
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
        let instances = vec![Some(Instance(0)), Some(Instance(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors = vec![None, Some(ColorRGBA(0xff000000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{:?}", df);
        //eprintln!("{:?}", expected);

        common::compare_df(
            &expected,
            &entity_view.as_df2::<Point2D, ColorRGBA>().unwrap(),
        );
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        let _used = entity_view;
    }
}

#[test]
fn no_instance_join_query() {
    let mut store = DataStore::new(Instance::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some points with an implicit instance
    let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
    let bundle = try_build_msg_bundle1(MsgId::random(), ent_path, timepoint, &points).unwrap();
    store.insert(&bundle).unwrap();

    // Assign them colors with explicit instances
    let colors = vec![ColorRGBA(0xff000000), ColorRGBA(0x00ff0000)];
    let bundle = try_build_msg_bundle1(MsgId::random(), ent_path, timepoint, &colors).unwrap();
    store.insert(&bundle).unwrap();

    // Retrieve the view
    let timeline_query = re_arrow_store::TimelineQuery::new(
        timepoint[0].0,
        TimeQuery::LatestAt(timepoint[0].1.as_i64()),
    );

    let entity_view = query_entity_with_primary(
        &store,
        &timeline_query,
        &ent_path.into(),
        Point2D::name(),
        &[ColorRGBA::name()],
    )
    .unwrap();

    // We expect this to generate the following `DataFrame`
    // ┌──────────┬───────────┬────────────┐
    // │ instance ┆ point2d   ┆ colorrgba  │
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
        let instances = vec![Some(Instance(0)), Some(Instance(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors = vec![Some(ColorRGBA(0xff000000)), Some(ColorRGBA(0x00ff0000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{:?}", df);
        //eprintln!("{:?}", expected);

        common::compare_df(
            &expected,
            &entity_view.as_df2::<Point2D, ColorRGBA>().unwrap(),
        );
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        let _used = entity_view;
    }
}

#[test]
fn missing_column_join_query() {
    let mut store = DataStore::new(Instance::name(), Default::default());

    let ent_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some points with an implicit instance
    let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
    let bundle = try_build_msg_bundle1(MsgId::random(), ent_path, timepoint, &points).unwrap();
    store.insert(&bundle).unwrap();

    // Retrieve the view
    let timeline_query = re_arrow_store::TimelineQuery::new(
        timepoint[0].0,
        TimeQuery::LatestAt(timepoint[0].1.as_i64()),
    );

    let entity_view = query_entity_with_primary(
        &store,
        &timeline_query,
        &ent_path.into(),
        Point2D::name(),
        &[ColorRGBA::name()],
    )
    .unwrap();

    // We expect this to generate the following `DataFrame`
    //
    // ┌──────────┬───────────┐
    // │ instance ┆ point2d   │
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
        let instances = vec![Some(Instance(0)), Some(Instance(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let expected = df_builder2(&instances, &points).unwrap();

        //eprintln!("{:?}", df);
        //eprintln!("{:?}", expected);

        common::compare_df(&expected, &entity_view.as_df1::<Point2D>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        let _used = entity_view;
    }
}
