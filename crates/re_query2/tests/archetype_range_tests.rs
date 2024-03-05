// TODO: rename range.rs

use itertools::{izip, Itertools};
use nohash_hasher::IntSet;
use re_query2::PromiseResolver;
use re_types::{components::InstanceKey, Archetype, ComponentName};
use smallvec::smallvec;

use re_data_store::{DataStore, TimeInt, TimeRange};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyPoint, MyPoints},
    DataCell, DataCellRow, DataRow, EntityPath, RowId,
};
use re_types_core::Loggable as _;

// ---

#[test]
fn simple_range() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let mut resolver = PromiseResolver::default();

    let entity_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123.into())];
    {
        // Create some points with implicit instances
        let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 2, points)
                .unwrap();
        store.insert_row(&row).unwrap();

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            entity_path.clone(),
            timepoint1,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint2 = [build_frame_nr(223.into())];
    {
        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(0)];
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            entity_path.clone(),
            timepoint2,
            1,
            (color_instances, colors),
        )
        .unwrap();
        store.insert_row(&row).unwrap();
    }

    let timepoint3 = [build_frame_nr(323.into())];
    {
        // Create some points with implicit instances
        let points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint3, 2, points)
                .unwrap();
        store.insert_row(&row).unwrap();
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    );

    let results = re_query2::range(
        &store,
        &query,
        &entity_path,
        MyPoints::all_components().iter().copied(),
    );

    let all_points = results.get_required::<MyPoint>().unwrap();
    let all_colors = results.get_optional::<MyColor>();

    let all_points = izip!(
        all_points.iter_indices(),
        all_points.iter_dense::<MyPoint>(&mut resolver)
    );
    let all_colors = izip!(
        all_colors.iter_indices(),
        all_colors.iter_sparse::<MyColor>(&mut resolver)
    );

    let mut results = re_query2::range_zip_1x1(all_points, all_colors);

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #323:
    // ┌─────────────┬──────────────┬─────────────────┐
    // │ InstanceKey ┆ MyPoint      ┆ MyColor         │
    // ╞═════════════╪══════════════╪═════════════════╡
    // │ 0           ┆ {10.0,20.0}  ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1           ┆ {30.0,40.0}  ┆ 4278190080      │
    // └─────────────┴──────────────┴─────────────────┘

    {
        // Frame #323

        let ((data_time, _row_id), points, colors) = results.next().unwrap();
        assert_eq!(Some(TimeInt::from(323)), data_time);

        let expected_points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let expected_colors = vec![
            Some(MyColor::from_rgb(255, 0, 0)),
            Some(MyColor::from_rgb(255, 0, 0)),
        ];

        let points = points.flatten().unwrap();
        let colors = colors.map_or(vec![], |colors| colors.flatten().unwrap());

        let (got_points, got_colors): (Vec<_>, Vec<_>) =
            re_query2::clamped_zip_1x1(points, colors, || None).unzip();

        similar_asserts::assert_eq!(expected_points, got_points);
        similar_asserts::assert_eq!(expected_colors, got_colors);
    }

    assert!(results.next().is_none());

    // --- Second test: `[timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    let results = re_query2::range(
        &store,
        &query,
        &entity_path,
        MyPoints::all_components().iter().copied(),
    );

    let all_points = results.get_required::<MyPoint>().unwrap();
    let all_colors = results.get_optional::<MyColor>();

    let all_points = izip!(
        all_points.iter_indices(),
        all_points.iter_dense::<MyPoint>(&mut resolver)
    );
    let all_colors = izip!(
        all_colors.iter_indices(),
        all_colors.iter_sparse::<MyColor>(&mut resolver)
    );

    let mut results = re_query2::range_zip_1x1(all_points, all_colors);

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey        ┆ MyPoint       ┆ MyColor         │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    // └────────────────────┴───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌────────────────────┬───────────────┬─────────────────┐
    // │ InstanceKey        ┆ MyPoint       ┆ MyColor         │
    // ╞════════════════════╪═══════════════╪═════════════════╡
    // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 1                  ┆ {30.0,40.0}   ┆ 4278190080      │
    // └────────────────────┴───────────────┴─────────────────┘

    {
        // Frame #123

        let ((data_time, _row_id), points, colors) = results.next().unwrap();
        assert_eq!(Some(TimeInt::from(123)), data_time);

        let expected_points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let expected_colors = vec![None, None];

        let points = points.flatten().unwrap();
        let colors = colors.map_or(vec![], |colors| colors.flatten().unwrap());

        let (got_points, got_colors): (Vec<_>, Vec<_>) =
            re_query2::clamped_zip_1x1(points, colors, || None).unzip();

        similar_asserts::assert_eq!(expected_points, got_points);
        similar_asserts::assert_eq!(expected_colors, got_colors);
    }
    {
        // Frame #323

        let ((data_time, _row_id), points, colors) = results.next().unwrap();
        assert_eq!(Some(TimeInt::from(323)), data_time);

        let expected_points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let expected_colors = vec![
            Some(MyColor::from_rgb(255, 0, 0)),
            Some(MyColor::from_rgb(255, 0, 0)),
        ];

        let points = points.flatten().unwrap();
        let colors = colors.map_or(vec![], |colors| colors.flatten().unwrap());

        let (got_points, got_colors): (Vec<_>, Vec<_>) =
            re_query2::clamped_zip_1x1(points, colors, || None).unzip();

        similar_asserts::assert_eq!(expected_points, got_points);
        similar_asserts::assert_eq!(expected_colors, got_colors);
    }

    assert!(results.next().is_none());
    // }
    //
    // #[test]
    // fn timeless_range() {
    //     let mut store = DataStore::new(
    //         re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
    //         InstanceKey::name(),
    //         Default::default(),
    //     );
    //
    //     let entity_path: EntityPath = "point".into();
    //
    //     let timepoint1 = [build_frame_nr(123.into())];
    //     {
    //         // Create some points with implicit instances
    //         let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    //         let mut row =
    //             DataRow::from_cells1(RowId::new(), entity_path.clone(), timepoint1, 2, &points)
    //                 .unwrap();
    //         row.compute_all_size_bytes();
    //         store.insert_row(&row).unwrap();
    //
    //         // Insert timelessly too!
    //         let row = DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), [], 2, &points)
    //             .unwrap();
    //         store.insert_row(&row).unwrap();
    //
    //         // Assign one of them a color with an explicit instance
    //         let color_instances = vec![InstanceKey(1)];
    //         let colors = vec![MyColor::from_rgb(255, 0, 0)];
    //         let row = DataRow::from_cells2_sized(
    //             RowId::new(),
    //             entity_path.clone(),
    //             timepoint1,
    //             1,
    //             (color_instances.clone(), colors.clone()),
    //         )
    //         .unwrap();
    //         store.insert_row(&row).unwrap();
    //
    //         // Insert timelessly too!
    //         let row = DataRow::from_cells2_sized(
    //             RowId::new(),
    //             entity_path.clone(),
    //             [],
    //             1,
    //             (color_instances, colors),
    //         )
    //         .unwrap();
    //         store.insert_row(&row).unwrap();
    //     }
    //
    //     let timepoint2 = [build_frame_nr(223.into())];
    //     {
    //         // Assign one of them a color with an explicit instance
    //         let color_instances = vec![InstanceKey(0)];
    //         let colors = vec![MyColor::from_rgb(255, 0, 0)];
    //         let row = DataRow::from_cells2_sized(
    //             RowId::new(),
    //             entity_path.clone(),
    //             timepoint2,
    //             1,
    //             (color_instances.clone(), colors.clone()),
    //         )
    //         .unwrap();
    //         store.insert_row(&row).unwrap();
    //
    //         // Insert timelessly too!
    //         let row = DataRow::from_cells2_sized(
    //             RowId::new(),
    //             entity_path.clone(),
    //             timepoint2,
    //             1,
    //             (color_instances, colors),
    //         )
    //         .unwrap();
    //         store.insert_row(&row).unwrap();
    //     }
    //
    //     let timepoint3 = [build_frame_nr(323.into())];
    //     {
    //         // Create some points with implicit instances
    //         let points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
    //         let row = DataRow::from_cells1_sized(
    //             RowId::new(),
    //             entity_path.clone(),
    //             timepoint3,
    //             2,
    //             &points,
    //         )
    //         .unwrap();
    //         store.insert_row(&row).unwrap();
    //
    //         // Insert timelessly too!
    //         let row = DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), [], 2, &points)
    //             .unwrap();
    //         store.insert_row(&row).unwrap();
    //     }
    //
    //     // ┌───────────┬──────────┬────────┬─────────────────┬────────────────────┬────────────────────┬────────────────────────────┐
    //     // │ insert_id ┆ frame_nr ┆ entity ┆ MyColor ┆ InstanceKey ┆ rerun.row_id       ┆ MyPoint              │
    //     // ╞═══════════╪══════════╪════════╪═════════════════╪════════════════════╪════════════════════╪════════════════════════════╡
    //     // │ 2         ┆ null     ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302243… ┆ [{1.0,2.0}, {3.0,4.0}]     │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 4         ┆ null     ┆ point  ┆ [4278190080]    ┆ [1]                ┆ [{167328063302246… ┆ null                       │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 8         ┆ null     ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302249… ┆ [{10.0,20.0}, {30.0,40.0}] │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1         ┆ 123      ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302236… ┆ [{1.0,2.0}, {3.0,4.0}]     │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 3         ┆ 123      ┆ point  ┆ [4278190080]    ┆ [1]                ┆ [{167328063302245… ┆ null                       │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 5         ┆ 223      ┆ point  ┆ [4278190080]    ┆ [0]                ┆ [{167328063302247… ┆ null                       │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 6         ┆ 223      ┆ point  ┆ [4278190080]    ┆ [0]                ┆ [{167328063302248… ┆ null                       │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 7         ┆ 323      ┆ point  ┆ null            ┆ [0, 1]             ┆ [{167328063302248… ┆ [{10.0,20.0}, {30.0,40.0}] │
    //     // └───────────┴──────────┴────────┴─────────────────┴────────────────────┴────────────────────┴────────────────────────────┘
    //
    //     // --- First test: `(timepoint1, timepoint3]` ---
    //
    //     let query = re_data_store::RangeQuery::new(
    //         timepoint1[0].0,
    //         TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    //     );
    //
    //     let arch_views =
    //         range_archetype::<MyPoints, { MyPoints::NUM_COMPONENTS }>(&store, &query, &entity_path);
    //
    //     let results = arch_views.collect::<Vec<_>>();
    //
    //     // We expect this to generate the following `DataFrame`s:
    //     //
    //     // Frame #323:
    //     // ┌────────────────────┬───────────────┬─────────────────┐
    //     // │ InstanceKey ┆ MyPoint ┆ MyColor │
    //     // ╞════════════════════╪═══════════════╪═════════════════╡
    //     // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    //     // └────────────────────┴───────────────┴─────────────────┘
    //
    //     {
    //         // Frame #323
    //
    //         let arch_view = &results[0];
    //         let time = arch_view.data_time().unwrap();
    //
    //         // Build expected df manually
    //         let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
    //         let points = vec![
    //             Some(MyPoint::new(10.0, 20.0)),
    //             Some(MyPoint::new(30.0, 40.0)),
    //         ];
    //         let colors = vec![Some(MyColor::from_rgb(255, 0, 0)), None];
    //         let expected = DataCellRow(smallvec![
    //             DataCell::from_native_sparse(instances),
    //             DataCell::from_native_sparse(points),
    //             DataCell::from_native_sparse(colors)
    //         ]);
    //
    //         //eprintln!("{expected:?}");
    //
    //         assert_eq!(TimeInt::from(323), time);
    //         assert_eq!(
    //             &expected,
    //             &arch_view.to_data_cell_row_2::<MyPoint, MyColor>().unwrap(),
    //         );
    //     }
    //
    //     // --- Second test: `[timepoint1, timepoint3]` ---
    //
    //     let query = re_data_store::RangeQuery::new(
    //         timepoint1[0].0,
    //         TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    //     );
    //
    //     let arch_views =
    //         range_archetype::<MyPoints, { MyPoints::NUM_COMPONENTS }>(&store, &query, &entity_path);
    //
    //     let results = arch_views.collect::<Vec<_>>();
    //
    //     // We expect this to generate the following `DataFrame`s:
    //     //
    //     // Frame #123:
    //     // ┌────────────────────┬───────────────┬─────────────────┐
    //     // │ InstanceKey ┆ MyPoint ┆ MyColor │
    //     // ╞════════════════════╪═══════════════╪═════════════════╡
    //     // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    //     // └────────────────────┴───────────────┴─────────────────┘
    //     //
    //     // Frame #323:
    //     // ┌────────────────────┬───────────────┬─────────────────┐
    //     // │ InstanceKey ┆ MyPoint ┆ MyColor │
    //     // ╞════════════════════╪═══════════════╪═════════════════╡
    //     // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    //     // └────────────────────┴───────────────┴─────────────────┘
    //
    //     {
    //         // Frame #123 (partially timeless)
    //
    //         let arch_view = &results[0];
    //         let time = arch_view.data_time().unwrap();
    //
    //         // Build expected df manually
    //         let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
    //         let points = vec![Some(MyPoint::new(1.0, 2.0)), Some(MyPoint::new(3.0, 4.0))];
    //         let colors = vec![None, Some(MyColor::from_rgb(255, 0, 0))];
    //         let expected = DataCellRow(smallvec![
    //             DataCell::from_native_sparse(instances),
    //             DataCell::from_native_sparse(points),
    //             DataCell::from_native_sparse(colors)
    //         ]);
    //
    //         //eprintln!("{expected:?}");
    //
    //         assert_eq!(TimeInt::from(123), time);
    //         assert_eq!(
    //             &expected,
    //             &arch_view.to_data_cell_row_2::<MyPoint, MyColor>().unwrap(),
    //         );
    //     }
    //     {
    //         // Frame #323
    //
    //         let arch_view = &results[1];
    //         let time = arch_view.data_time().unwrap();
    //
    //         // Build expected df manually
    //         let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
    //         let points = vec![
    //             Some(MyPoint::new(10.0, 20.0)),
    //             Some(MyPoint::new(30.0, 40.0)),
    //         ];
    //         let colors = vec![Some(MyColor::from_rgb(255, 0, 0)), None];
    //         let expected = DataCellRow(smallvec![
    //             DataCell::from_native_sparse(instances),
    //             DataCell::from_native_sparse(points),
    //             DataCell::from_native_sparse(colors)
    //         ]);
    //
    //         //eprintln!("{expected:?}");
    //
    //         assert_eq!(TimeInt::from(323), time);
    //         assert_eq!(
    //             &expected,
    //             &arch_view.to_data_cell_row_2::<MyPoint, MyColor>().unwrap(),
    //         );
    //     }
    //
    //     // --- Third test: `[-inf, +inf]` ---
    //
    //     let query =
    //         re_data_store::RangeQuery::new(timepoint1[0].0, TimeRange::new(TimeInt::MIN, TimeInt::MAX));
    //
    //     let arch_views =
    //         range_archetype::<MyPoints, { MyPoints::NUM_COMPONENTS }>(&store, &query, &entity_path);
    //
    //     let results = arch_views.collect::<Vec<_>>();
    //
    //     // We expect this to generate the following `DataFrame`s:
    //     //
    //     // Timeless #1:
    //     // ┌────────────────────┬───────────────┬─────────────────┐
    //     // │ InstanceKey ┆ MyPoint ┆ MyColor │
    //     // ╞════════════════════╪═══════════════╪═════════════════╡
    //     // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    //     // └────────────────────┴───────────────┴─────────────────┘
    //     //
    //     // Timeless #2:
    //     // ┌────────────────────┬───────────────┬─────────────────┐
    //     // │ InstanceKey ┆ MyPoint ┆ MyColor │
    //     // ╞════════════════════╪═══════════════╪═════════════════╡
    //     // │ 0                  ┆ {10.0,20.0}   ┆ null            │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1                  ┆ {30.0,40.0}   ┆ 4278190080      │
    //     // └────────────────────┴───────────────┴─────────────────┘
    //     //
    //     // Frame #123:
    //     // ┌────────────────────┬───────────────┬─────────────────┐
    //     // │ InstanceKey ┆ MyPoint ┆ MyColor │
    //     // ╞════════════════════╪═══════════════╪═════════════════╡
    //     // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1                  ┆ {3.0,4.0}     ┆ 4278190080      │
    //     // └────────────────────┴───────────────┴─────────────────┘
    //     //
    //     // Frame #323:
    //     // ┌────────────────────┬───────────────┬─────────────────┐
    //     // │ InstanceKey ┆ MyPoint ┆ MyColor │
    //     // ╞════════════════════╪═══════════════╪═════════════════╡
    //     // │ 0                  ┆ {10.0,20.0}   ┆ 4278190080      │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1                  ┆ {30.0,40.0}   ┆ null            │
    //     // └────────────────────┴───────────────┴─────────────────┘
    //
    //     {
    //         // Timeless #1
    //
    //         let arch_view = &results[0];
    //         let time = arch_view.data_time();
    //
    //         // Build expected df manually
    //         let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
    //         let points = vec![Some(MyPoint::new(1.0, 2.0)), Some(MyPoint::new(3.0, 4.0))];
    //         let colors: Vec<Option<MyColor>> = vec![None, None];
    //         let expected = DataCellRow(smallvec![
    //             DataCell::from_native_sparse(instances),
    //             DataCell::from_native_sparse(points),
    //             DataCell::from_native_sparse(colors)
    //         ]);
    //
    //         //eprintln!("{expected:?}");
    //
    //         assert_eq!(None, time);
    //         assert_eq!(
    //             &expected,
    //             &arch_view.to_data_cell_row_2::<MyPoint, MyColor>().unwrap(),
    //         );
    //
    //         // Timeless #2
    //
    //         let arch_view = &results[1];
    //         let time = arch_view.data_time();
    //
    //         // Build expected df manually
    //         let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
    //         let points = vec![
    //             Some(MyPoint::new(10.0, 20.0)),
    //             Some(MyPoint::new(30.0, 40.0)),
    //         ];
    //         let colors = vec![None, Some(MyColor::from_rgb(255, 0, 0))];
    //         let expected = DataCellRow(smallvec![
    //             DataCell::from_native_sparse(instances),
    //             DataCell::from_native_sparse(points),
    //             DataCell::from_native_sparse(colors)
    //         ]);
    //
    //         //eprintln!("{expected:?}");
    //
    //         assert_eq!(None, time);
    //         assert_eq!(
    //             &expected,
    //             &arch_view.to_data_cell_row_2::<MyPoint, MyColor>().unwrap(),
    //         );
    //
    //         // Frame #123 (partially timeless)
    //
    //         let arch_view = &results[2];
    //         let time = arch_view.data_time().unwrap();
    //
    //         // Build expected df manually
    //         let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
    //         let points = vec![Some(MyPoint::new(1.0, 2.0)), Some(MyPoint::new(3.0, 4.0))];
    //         let colors = vec![None, Some(MyColor::from_rgb(255, 0, 0))];
    //         let expected = DataCellRow(smallvec![
    //             DataCell::from_native_sparse(instances),
    //             DataCell::from_native_sparse(points),
    //             DataCell::from_native_sparse(colors)
    //         ]);
    //
    //         //eprintln!("{expected:?}");
    //
    //         assert_eq!(TimeInt::from(123), time);
    //         assert_eq!(
    //             &expected,
    //             &arch_view.to_data_cell_row_2::<MyPoint, MyColor>().unwrap(),
    //         );
    //     }
    //     {
    //         // Frame #323
    //
    //         let arch_view = &results[3];
    //         let time = arch_view.data_time().unwrap();
    //
    //         // Build expected df manually
    //         let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
    //         let points = vec![
    //             Some(MyPoint::new(10.0, 20.0)),
    //             Some(MyPoint::new(30.0, 40.0)),
    //         ];
    //         let colors = vec![Some(MyColor::from_rgb(255, 0, 0)), None];
    //         let expected = DataCellRow(smallvec![
    //             DataCell::from_native_sparse(instances),
    //             DataCell::from_native_sparse(points),
    //             DataCell::from_native_sparse(colors)
    //         ]);
    //
    //         //eprintln!("{expected:?}");
    //
    //         assert_eq!(TimeInt::from(323), time);
    //         assert_eq!(
    //             &expected,
    //             &arch_view.to_data_cell_row_2::<MyPoint, MyColor>().unwrap(),
    //         );
    //     }
    // }
    //
    // #[test]
    // fn simple_splatted_range() {
    //     let mut store = DataStore::new(
    //         re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
    //         InstanceKey::name(),
    //         Default::default(),
    //     );
    //
    //     let entity_path: EntityPath = "point".into();
    //
    //     let timepoint1 = [build_frame_nr(123.into())];
    //     {
    //         // Create some points with implicit instances
    //         let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    //         let row =
    //             DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 2, points)
    //                 .unwrap();
    //         store.insert_row(&row).unwrap();
    //
    //         // Assign one of them a color with an explicit instance
    //         let color_instances = vec![InstanceKey(1)];
    //         let colors = vec![MyColor::from_rgb(255, 0, 0)];
    //         let row = DataRow::from_cells2_sized(
    //             RowId::new(),
    //             entity_path.clone(),
    //             timepoint1,
    //             1,
    //             (color_instances, colors),
    //         )
    //         .unwrap();
    //         store.insert_row(&row).unwrap();
    //     }
    //
    //     let timepoint2 = [build_frame_nr(223.into())];
    //     {
    //         // Assign one of them a color with a splatted instance
    //         let color_instances = vec![InstanceKey::SPLAT];
    //         let colors = vec![MyColor::from_rgb(0, 255, 0)];
    //         let row = DataRow::from_cells2_sized(
    //             RowId::new(),
    //             entity_path.clone(),
    //             timepoint2,
    //             1,
    //             (color_instances, colors),
    //         )
    //         .unwrap();
    //         store.insert_row(&row).unwrap();
    //     }
    //
    //     let timepoint3 = [build_frame_nr(323.into())];
    //     {
    //         // Create some points with implicit instances
    //         let points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
    //         let row =
    //             DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint3, 2, points)
    //                 .unwrap();
    //         store.insert_row(&row).unwrap();
    //     }
    //
    //     // --- First test: `(timepoint1, timepoint3]` ---
    //
    //     let query = re_data_store::RangeQuery::new(
    //         timepoint1[0].0,
    //         TimeRange::new((timepoint1[0].1.as_i64() + 1).into(), timepoint3[0].1),
    //     );
    //
    //     let arch_views =
    //         range_archetype::<MyPoints, { MyPoints::NUM_COMPONENTS }>(&store, &query, &entity_path);
    //
    //     let results = arch_views.collect::<Vec<_>>();
    //
    //     // We expect this to generate the following `DataFrame`s:
    //     //
    //     // Frame #323:
    //     // ┌────────────────────┬───────────────┬─────────────────┐
    //     // │ InstanceKey ┆ MyPoint ┆ MyColor │
    //     // ╞════════════════════╪═══════════════╪═════════════════╡
    //     // │ 0                  ┆ {10.0,20.0}   ┆ 16711680        │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1                  ┆ {30.0,40.0}   ┆ 16711680        │
    //     // └────────────────────┴───────────────┴─────────────────┘
    //
    //     assert_eq!(results.len(), 1);
    //
    //     {
    //         // Frame #323
    //
    //         let arch_view = &results[0];
    //         let time = arch_view.data_time().unwrap();
    //
    //         // Build expected df manually
    //         let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
    //         let points = vec![
    //             Some(MyPoint::new(10.0, 20.0)),
    //             Some(MyPoint::new(30.0, 40.0)),
    //         ];
    //         let colors = vec![
    //             Some(MyColor::from_rgb(0, 255, 0)),
    //             Some(MyColor::from_rgb(0, 255, 0)),
    //         ];
    //
    //         let df = arch_view.to_data_cell_row_2::<MyPoint, MyColor>().unwrap();
    //         let expected = DataCellRow(smallvec![
    //             DataCell::from_native_sparse(instances),
    //             DataCell::from_native_sparse(points),
    //             DataCell::from_native_sparse(colors)
    //         ]);
    //
    //         //eprintln!("{expected:?}");
    //
    //         assert_eq!(TimeInt::from(323), time);
    //         assert_eq!(&expected, &df);
    //     }
    //
    //     // --- Second test: `[timepoint1, timepoint3]` ---
    //
    //     let query = re_data_store::RangeQuery::new(
    //         timepoint1[0].0,
    //         TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    //     );
    //
    //     let arch_views =
    //         range_archetype::<MyPoints, { MyPoints::NUM_COMPONENTS }>(&store, &query, &entity_path);
    //
    //     let results = arch_views.collect::<Vec<_>>();
    //
    //     // We expect this to generate the following `DataFrame`s:
    //     //
    //     // Frame #123:
    //     // ┌────────────────────┬───────────────┬─────────────────┐
    //     // │ InstanceKey ┆ MyPoint ┆ MyColor │
    //     // ╞════════════════════╪═══════════════╪═════════════════╡
    //     // │ 0                  ┆ {1.0,2.0}     ┆ null            │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1                  ┆ {3.0,4.0}     ┆ null            │
    //     // └────────────────────┴───────────────┴─────────────────┘
    //     //
    //     // Frame #323:
    //     // ┌────────────────────┬───────────────┬─────────────────┐
    //     // │ InstanceKey ┆ MyPoint ┆ MyColor │
    //     // ╞════════════════════╪═══════════════╪═════════════════╡
    //     // │ 0                  ┆ {10.0,20.0}   ┆ 16711680        │
    //     // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    //     // │ 1                  ┆ {30.0,40.0}   ┆ 16711680        │
    //     // └────────────────────┴───────────────┴─────────────────┘
    //
    //     {
    //         // Frame #123
    //
    //         let arch_view = &results[0];
    //         let time = arch_view.data_time().unwrap();
    //
    //         // Build expected df manually
    //         let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
    //         let points = vec![Some(MyPoint::new(1.0, 2.0)), Some(MyPoint::new(3.0, 4.0))];
    //         let colors: Vec<Option<MyColor>> = vec![None, None];
    //         let expected = DataCellRow(smallvec![
    //             DataCell::from_native_sparse(instances),
    //             DataCell::from_native_sparse(points),
    //             DataCell::from_native_sparse(colors)
    //         ]);
    //
    //         //eprintln!("{expected:?}");
    //
    //         assert_eq!(TimeInt::from(123), time);
    //         assert_eq!(
    //             &expected,
    //             &arch_view.to_data_cell_row_2::<MyPoint, MyColor>().unwrap(),
    //         );
    //     }
    //     {
    //         // Frame #323
    //
    //         let arch_view = &results[1];
    //         let time = arch_view.data_time().unwrap();
    //
    //         // Build expected df manually
    //         let instances = vec![Some(InstanceKey(0)), Some(InstanceKey(1))];
    //         let points = vec![
    //             Some(MyPoint::new(10.0, 20.0)),
    //             Some(MyPoint::new(30.0, 40.0)),
    //         ];
    //         let colors = vec![
    //             Some(MyColor::from_rgb(0, 255, 0)),
    //             Some(MyColor::from_rgb(0, 255, 0)),
    //         ];
    //         let expected = DataCellRow(smallvec![
    //             DataCell::from_native_sparse(instances),
    //             DataCell::from_native_sparse(points),
    //             DataCell::from_native_sparse(colors)
    //         ]);
    //
    //         //eprintln!("{expected:?}");
    //
    //         assert_eq!(TimeInt::from(323), time);
    //         assert_eq!(
    //             &expected,
    //             &arch_view.to_data_cell_row_2::<MyPoint, MyColor>().unwrap(),
    //         );
    //     }
}
