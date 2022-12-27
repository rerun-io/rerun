mod common;

use re_arrow_store::{DataStore, TimeInt, TimeRange};
use re_log_types::{
    datagen::build_frame_nr,
    field_types::Instance,
    field_types::{ColorRGBA, Point2D},
    msg_bundle::try_build_msg_bundle1,
    msg_bundle::try_build_msg_bundle2,
    msg_bundle::Component,
    MsgId, ObjPath,
};
use re_query::range_entity_with_primary;

#[test]
fn simple_range() {
    let mut store = DataStore::new(Instance::name(), Default::default());

    let ent_path: ObjPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some points with implicit instances
        let points = vec![Point2D { x: 1.0, y: 2.0 }, Point2D { x: 3.0, y: 4.0 }];
        let bundle =
            try_build_msg_bundle1(MsgId::random(), ent_path.clone(), timepoint1, &points).unwrap();
        store.insert(&bundle).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![Instance(1)];
        let colors = vec![ColorRGBA(0xff000000)];
        let bundle = try_build_msg_bundle2(
            MsgId::random(),
            ent_path.clone(),
            timepoint1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert(&bundle).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with an explicit instance
        let color_instances = vec![Instance(0)];
        let colors = vec![ColorRGBA(0xff000000)];
        let bundle = try_build_msg_bundle2(
            MsgId::random(),
            ent_path.clone(),
            timepoint2,
            (color_instances, colors),
        )
        .unwrap();
        store.insert(&bundle).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some points with implicit instances
        let points = vec![Point2D { x: 10.0, y: 20.0 }, Point2D { x: 30.0, y: 40.0 }];
        let bundle =
            try_build_msg_bundle1(MsgId::random(), ent_path.clone(), timepoint3, &points).unwrap();
        store.insert(&bundle).unwrap();
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    // The exclusion of `timepoint1` means latest-at semantics will kick in!

    let query = re_arrow_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    let components = &[ColorRGBA::name()];
    let ent_views =
        range_entity_with_primary(&store, &query, &ent_path, Point2D::name(), components);

    let results = ent_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance ┆ rerun.point2d ┆ rerun.colorrgba │
    // │ ---            ┆ ---           ┆ ---             │
    // │ u64            ┆ struct[2]     ┆ u32             │
    // ╞════════════════╪═══════════════╪═════════════════╡
    // │ 0              ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1              ┆ {3.0,4.0}     ┆ 4278190080      │
    // └────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance ┆ rerun.point2d ┆ rerun.colorrgba │
    // │ ---            ┆ ---           ┆ ---             │
    // │ u64            ┆ struct[2]     ┆ u32             │
    // ╞════════════════╪═══════════════╪═════════════════╡
    // │ 0              ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1              ┆ {30.0,40.0}   ┆ null            │
    // └────────────────┴───────────────┴─────────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Frame #123

        let (time, ent_view) = &results[0];

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

        assert_eq!(TimeInt::from(123), *time);
        common::compare_df(&expected, &ent_view.as_df2::<Point2D, ColorRGBA>().unwrap());

        // Frame #323

        let (time, ent_view) = &results[1];

        // Build expected df manually
        let instances = vec![Some(Instance(0)), Some(Instance(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![Some(ColorRGBA(0xff000000)), None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{:?}", df);
        //eprintln!("{:?}", expected);

        assert_eq!(TimeInt::from(323), *time);
        common::compare_df(&expected, &ent_view.as_df2::<Point2D, ColorRGBA>().unwrap());
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

    let components = &[ColorRGBA::name()];
    let ent_views =
        range_entity_with_primary(&store, &query, &ent_path, Point2D::name(), components);

    let results = ent_views.collect::<Vec<_>>();

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance ┆ rerun.point2d ┆ rerun.colorrgba │
    // │ ---            ┆ ---           ┆ ---             │
    // │ u64            ┆ struct[2]     ┆ u32             │
    // ╞════════════════╪═══════════════╪═════════════════╡
    // │ 0              ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1              ┆ {3.0,4.0}     ┆ null            │
    // └────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────┬───────────────┬─────────────────┐
    // │ rerun.instance ┆ rerun.point2d ┆ rerun.colorrgba │
    // │ ---            ┆ ---           ┆ ---             │
    // │ u64            ┆ struct[2]     ┆ u32             │
    // ╞════════════════╪═══════════════╪═════════════════╡
    // │ 0              ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1              ┆ {30.0,40.0}   ┆ null            │
    // └────────────────┴───────────────┴─────────────────┘

    #[cfg(feature = "polars")]
    {
        use re_query::dataframe_util::df_builder3;

        // Frame #123

        let (time, ent_view) = &results[0];

        // Build expected df manually
        let instances = vec![Some(Instance(0)), Some(Instance(1))];
        let points = vec![
            Some(Point2D { x: 1.0, y: 2.0 }),
            Some(Point2D { x: 3.0, y: 4.0 }),
        ];
        let colors: Vec<Option<ColorRGBA>> = vec![None, None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{:?}", df);
        //eprintln!("{:?}", expected);

        assert_eq!(TimeInt::from(123), *time);
        common::compare_df(&expected, &ent_view.as_df2::<Point2D, ColorRGBA>().unwrap());

        // Frame #323

        let (time, ent_view) = &results[1];

        // Build expected df manually
        let instances = vec![Some(Instance(0)), Some(Instance(1))];
        let points = vec![
            Some(Point2D { x: 10.0, y: 20.0 }),
            Some(Point2D { x: 30.0, y: 40.0 }),
        ];
        let colors = vec![Some(ColorRGBA(0xff000000)), None];
        let expected = df_builder3(&instances, &points, &colors).unwrap();

        //eprintln!("{:?}", df);
        //eprintln!("{:?}", expected);

        assert_eq!(TimeInt::from(323), *time);
        common::compare_df(&expected, &ent_view.as_df2::<Point2D, ColorRGBA>().unwrap());
    }
    #[cfg(not(feature = "polars"))]
    {
        //TODO(jleibs): non-polars test validation
        _ = results;
    }
}
