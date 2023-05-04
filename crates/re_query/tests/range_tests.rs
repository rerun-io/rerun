mod common;

use re_arrow_store::{DataStore, TimeInt, TimeRange};
use re_log_types::{
    component_types::InstanceKey,
    component_types::{ColorRGBA, Point2D},
    datagen::build_frame_nr,
    Component, DataRow, EntityPath, RowId,
};
use re_query::range_entity_with_primary;

#[test]
fn simple_range() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some points with implicit instances
        let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
        let row =
            DataRow::from_cells1_sized(RowId::random(), ent_path.clone(), timepoint1, 2, points);
        store.insert_row(&row).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![ColorRGBA(0xff000000)];
        let row = DataRow::from_cells2_sized(
            RowId::random(),
            ent_path.clone(),
            timepoint1,
            1,
            (color_instances, colors),
        );
        store.insert_row(&row).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(0)];
        let colors = vec![ColorRGBA(0xff000000)];
        let row = DataRow::from_cells2_sized(
            RowId::random(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances, colors),
        );
        store.insert_row(&row).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some points with implicit instances
        let points = vec![Point2D { x: 10.0, y: 20.0 }, Point2D { x: 30.0, y: 40.0 }];
        let row =
            DataRow::from_cells1_sized(RowId::random(), ent_path.clone(), timepoint3, 2, points);
        store.insert_row(&row).unwrap();
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    // The exclusion of `timepoint1` means latest-at semantics will kick in!

    let query = re_arrow_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    let components = [InstanceKey::name(), Point2D::name(), ColorRGBA::name()];
    let ent_views = range_entity_with_primary::<Point2D, 3>(&store, &query, &ent_path, components);

    let results = ent_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Frame #123

        let (time, ent_view) = &results[0];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors = vec![None, Some(ColorRGBA(0xff000000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());

        // Frame #323

        let (time, ent_view) = &results[1];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![Some(ColorRGBA(0xff000000)), None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        _ = results;
    }

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will _not_ kick in!

    let query = re_arrow_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    let components = [InstanceKey::name(), Point2D::name(), ColorRGBA::name()];
    let ent_views = range_entity_with_primary::<Point2D, 3>(&store, &query, &ent_path, components);

    let results = ent_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Frame #123

        let (time, ent_view) = &results[0];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors: Vec<Option<ColorRGBA>> = vec![None, None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());

        // Frame #323

        let (time, ent_view) = &results[1];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![Some(ColorRGBA(0xff000000)), None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        _ = results;
    }
}

#[test]
fn timeless_range() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some points with implicit instances
        let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
        let mut row =
            DataRow::from_cells1(RowId::random(), ent_path.clone(), timepoint1, 2, &points);
        row.compute_all_size_bytes();
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row = DataRow::from_cells1_sized(RowId::random(), ent_path.clone(), [], 2, &points);
        store.insert_row(&row).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![ColorRGBA(0xff000000)];
        let row = DataRow::from_cells2_sized(
            RowId::random(),
            ent_path.clone(),
            timepoint1,
            1,
            (color_instances.clone(), colors.clone()),
        );
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row = DataRow::from_cells2_sized(
            RowId::random(),
            ent_path.clone(),
            [],
            1,
            (color_instances, colors),
        );
        store.insert_row(&row).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(0)];
        let colors = vec![ColorRGBA(0xff000000)];
        let row = DataRow::from_cells2_sized(
            RowId::random(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances.clone(), colors.clone()),
        );
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row = DataRow::from_cells2_sized(
            RowId::random(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances, colors),
        );
        store.insert_row(&row).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some points with implicit instances
        let points = vec![Point2D { x: 10.0, y: 20.0 }, Point2D { x: 30.0, y: 40.0 }];
        let row =
            DataRow::from_cells1_sized(RowId::random(), ent_path.clone(), timepoint3, 2, &points);
        store.insert_row(&row).unwrap();

        // Insert timelessly too!
        let row = DataRow::from_cells1_sized(RowId::random(), ent_path.clone(), [], 2, &points);
        store.insert_row(&row).unwrap();
    }

    // ┌───────────┬──────────┬────────┬─────────────────┬────────────────────┬──────────────────────┬────────────────────────────┐
    // │ insert_id ┆ frame_nr ┆ entity ┆ rerun.colorrgba ┆ rerun.instance_key ┆ rerun.row_id         ┆ rerun.point2d              │
    // ╞═══════════╪══════════╪════════╪═════════════════╪════════════════════╪══════════════════════╪════════════════════════════╡
    // │ 2         ┆ null     ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302243... ┆ [{1.0,2.0}, {3.0,4.0}]     │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 4         ┆ null     ┆ point  ┆ [4278190080]    ┆ [1]                ┆ [{167328063302246... ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 8         ┆ null     ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302249... ┆ [{10.0,20.0}, {30.0,40.0}] │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1         ┆ 123      ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302236... ┆ [{1.0,2.0}, {3.0,4.0}]     │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 3         ┆ 123      ┆ point  ┆ [4278190080]    ┆ [1]                ┆ [{167328063302245... ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 5         ┆ 223      ┆ point  ┆ [4278190080]    ┆ [0]                ┆ [{167328063302247... ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 6         ┆ 223      ┆ point  ┆ [4278190080]    ┆ [0]                ┆ [{167328063302248... ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 7         ┆ 323      ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302248... ┆ [{10.0,20.0}, {30.0,40.0}] │
    // └───────────┴──────────┴────────┴─────────────────┴────────────────────┴──────────────────────┴────────────────────────────┘

    // --- First test: `(timepoint1, timepoint3]` ---

    // The exclusion of `timepoint1` means latest-at semantics will kick in!

    let query = re_arrow_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    let components = [InstanceKey::name(), Point2D::name(), ColorRGBA::name()];
    let ent_views = range_entity_with_primary::<Point2D, 3>(&store, &query, &ent_path, components);

    let results = ent_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Frame #123

        let (time, ent_view) = &results[0];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors = vec![None, Some(ColorRGBA(0xff000000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());

        // Frame #323

        let (time, ent_view) = &results[1];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![Some(ColorRGBA(0xff000000)), None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        _ = results;
    }

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will fall back to timeless data!

    let query = re_arrow_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    let components = [InstanceKey::name(), Point2D::name(), ColorRGBA::name()];
    let ent_views = range_entity_with_primary::<Point2D, 3>(&store, &query, &ent_path, components);

    let results = ent_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #122:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Frame #122 (all timeless)

        let (time, ent_view) = &results[0];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![None, Some(ColorRGBA(0xff000000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(122), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());

        // Frame #123 (partially timeless)

        let (time, ent_view) = &results[1];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors = vec![None, Some(ColorRGBA(0xff000000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());

        // Frame #323

        let (time, ent_view) = &results[2];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![Some(ColorRGBA(0xff000000)), None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        _ = results;
    }

    // --- Third test: `[-inf, +inf]` ---

    let query = re_arrow_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(TimeInt::MIN, TimeInt::MAX),
    );

    let components = [InstanceKey::name(), Point2D::name(), ColorRGBA::name()];
    let ent_views = range_entity_with_primary::<Point2D, 3>(&store, &query, &ent_path, components);

    let results = ent_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Timeless #1:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Timeless #2:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Timeless #1

        let (time, ent_view) = &results[0];

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors: Vec<Option<ColorRGBA>> = vec![None, None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(&None, time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());

        // Timeless #2

        let (time, ent_view) = &results[1];

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![None, Some(ColorRGBA(0xff000000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(&None, time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());

        // Frame #123 (partially timeless)

        let (time, ent_view) = &results[2];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors = vec![None, Some(ColorRGBA(0xff000000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());

        // Frame #323

        let (time, ent_view) = &results[3];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![Some(ColorRGBA(0xff000000)), None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        _ = results;
    }
}

#[test]
fn simple_splatted_range() {
    let mut store = DataStore::new(InstanceKey::name(), Default::default());

    let ent_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some points with implicit instances
        let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
        let row =
            DataRow::from_cells1_sized(RowId::random(), ent_path.clone(), timepoint1, 2, points);
        store.insert_row(&row).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![ColorRGBA(0xff000000)];
        let row = DataRow::from_cells2_sized(
            RowId::random(),
            ent_path.clone(),
            timepoint1,
            1,
            (color_instances, colors),
        );
        store.insert_row(&row).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with a splatted instance
        let color_instances = vec![InstanceKey::SPLAT];
        let colors = vec![ColorRGBA(0x00ff0000)];
        let row = DataRow::from_cells2_sized(
            RowId::random(),
            ent_path.clone(),
            timepoint2,
            1,
            (color_instances, colors),
        );
        store.insert_row(&row).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some points with implicit instances
        let points = vec![Point2D { x: 10.0, y: 20.0 }, Point2D { x: 30.0, y: 40.0 }];
        let row =
            DataRow::from_cells1_sized(RowId::random(), ent_path.clone(), timepoint3, 2, points);
        store.insert_row(&row).unwrap();
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    // The exclusion of `timepoint1` means latest-at semantics will kick in!

    let query = re_arrow_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    let components = [InstanceKey::name(), Point2D::name(), ColorRGBA::name()];
    let ent_views = range_entity_with_primary::<Point2D, 3>(&store, &query, &ent_path, components);

    let results = ent_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 16711680        │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ 16711680        │
    // └────────────────────┴───────────────┴─────────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Frame #123

        let (time, ent_view) = &results[0];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors = vec![None, Some(ColorRGBA(0xff000000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());

        // Frame #323

        let (time, ent_view) = &results[1];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![Some(ColorRGBA(0x00ff0000)), Some(ColorRGBA(0x00ff0000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        _ = results;
    }

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will _not_ kick in!

    let query = re_arrow_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    let components = [InstanceKey::name(), Point2D::name(), ColorRGBA::name()];
    let ent_views = range_entity_with_primary::<Point2D, 3>(&store, &query, &ent_path, components);

    let results = ent_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance_key ┆ rerun.point2d ┆ rerun.colorrgba │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 16711680        │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ 16711680        │
    // └────────────────────┴───────────────┴─────────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Frame #123

        let (time, ent_view) = &results[0];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors: Vec<Option<ColorRGBA>> = vec![None, None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(123), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());

        // Frame #323

        let (time, ent_view) = &results[1];
        let time = time.unwrap();

        // Build expected df manually
        let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![Some(ColorRGBA(0x00ff0000)), Some(ColorRGBA(0x00ff0000))];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{df:?}");
        //eprintln!("{expected:?}");

        assert_eq!(TimeInt::from(323), time);
        common::compare_df(&expected, &ent_view.as_df2::<ColorRGBA>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        _ = results;
    }
}
