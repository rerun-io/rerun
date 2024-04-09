use itertools::izip;
use re_query2::PromiseResolver;
use re_types::{components::InstanceKey, Archetype};

use re_data_store::{DataStore, TimeInt, TimeRange};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyPoint, MyPoints},
    DataRow, EntityPath, RowId, TimePoint,
};
use re_types_core::Loggable as _;

// ---

#[test]
fn simple_range() -> anyhow::Result<()> {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let resolver = PromiseResolver::default();

    let entity_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123)];
    {
        let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 2, points)?;
        store.insert_row(&row)?;

        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 1, colors)?;
        store.insert_row(&row)?;
    }

    let timepoint2 = [build_frame_nr(223)];
    {
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint2, 1, colors)?;
        store.insert_row(&row)?;
    }

    let timepoint3 = [build_frame_nr(323)];
    {
        let points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint3, 2, points)?;
        store.insert_row(&row)?;
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1.as_i64() + 1, timepoint3[0].1),
    );

    let results = re_query2::range(
        &store,
        &query,
        &entity_path,
        MyPoints::all_components().iter().copied(),
    );

    let all_points = results.get_required(MyPoint::name())?;
    let all_colors = results.get_or_empty(MyColor::name());

    let all_points = izip!(
        all_points.iter_indices(),
        all_points.iter_dense::<MyPoint>(&resolver)
    );
    let all_colors = izip!(
        all_colors.iter_indices(),
        all_colors.iter_sparse::<MyColor>(&resolver)
    );

    let mut results = re_query2::range_zip_1x1(all_points, all_colors);

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #323:
    // ┌──────────────┬─────────────────┐
    // │ MyPoint      ┆ MyColor         │
    // ╞══════════════╪═════════════════╡
    // │ {10.0,20.0}  ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {30.0,40.0}  ┆ 4278190080      │
    // └──────────────┴─────────────────┘

    {
        // Frame #323

        let ((data_time, _row_id), points, colors) = results.next().unwrap();
        assert_eq!(TimeInt::new_temporal(323), data_time);

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

    let all_points = results.get_required(MyPoint::name())?;
    let all_colors = results.get_or_empty(MyColor::name());

    let all_points = izip!(
        all_points.iter_indices(),
        all_points.iter_dense::<MyPoint>(&resolver)
    );
    let all_colors = izip!(
        all_colors.iter_indices(),
        all_colors.iter_sparse::<MyColor>(&resolver)
    );

    let mut results = re_query2::range_zip_1x1(all_points, all_colors);

    // We expect this to generate the following `DataFrame`s:
    //
    // Frame #123:
    // ┌───────────────┬─────────────────┐
    // │ MyPoint       ┆ MyColor         │
    // ╞═══════════════╪═════════════════╡
    // │ {1.0,2.0}     ┆ null            │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {3.0,4.0}     ┆ null            │
    // └───────────────┴─────────────────┘
    //
    // Frame #323:
    // ┌───────────────┬─────────────────┐
    // │ MyPoint       ┆ MyColor         │
    // ╞═══════════════╪═════════════════╡
    // │ {10.0,20.0}   ┆ 4278190080      │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {30.0,40.0}   ┆ 4278190080      │
    // └───────────────┴─────────────────┘

    {
        // Frame #123

        let ((data_time, _row_id), points, colors) = results.next().unwrap();
        assert_eq!(TimeInt::new_temporal(123), data_time);

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
        assert_eq!(TimeInt::new_temporal(323), data_time);

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

    Ok(())
}

#[test]
fn static_range() -> anyhow::Result<()> {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let resolver = PromiseResolver::default();

    let entity_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123)];
    {
        // Create some Positions with implicit instances
        let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let mut row =
            DataRow::from_cells1(RowId::new(), entity_path.clone(), timepoint1, 2, positions)?;
        row.compute_all_size_bytes();
        store.insert_row(&row)?;

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            entity_path.clone(),
            timepoint1,
            1,
            (color_instances.clone(), colors.clone()),
        )?;
        store.insert_row(&row)?;
    }

    let timepoint2 = [build_frame_nr(223)];
    {
        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(0)];
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            entity_path.clone(),
            timepoint2,
            1,
            (color_instances.clone(), colors.clone()),
        )?;
        store.insert_row(&row)?;

        // Insert statically too!
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path.clone(),
            TimePoint::default(),
            1,
            colors,
        )?;
        store.insert_row(&row)?;
    }

    let timepoint3 = [build_frame_nr(323)];
    {
        // Create some Positions with implicit instances
        let positions = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path.clone(),
            timepoint3,
            2,
            positions,
        )?;
        store.insert_row(&row)?;
    }

    // ┌──────────┬───────────────┬────────────────────────────┐
    // │ frame_nr ┆ MyColor       ┆ MyColor                    │
    // ╞══════════╪═══════════════╪════════════════════════════╡
    // │ null     ┆ [4278190080]  ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 123      ┆ null          ┆ [{1.0,2.0}, {3.0,4.0}]     │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 123      ┆ [4278190080]  ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 223      ┆ [4278190080]  ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 223      ┆ [4278190080]  ┆ null                       │
    // ├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ 323      ┆ null          ┆ [{10.0,20.0}, {30.0,40.0}] │
    // └──────────┴───────────────┴────────────────────────────┘

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1.as_i64() + 1, timepoint3[0].1),
    );

    let results = re_query2::range(
        &store,
        &query,
        &entity_path,
        MyPoints::all_components().iter().copied(),
    );

    let all_points = results.get_required(MyPoint::name())?;
    let all_colors = results.get_or_empty(MyColor::name());

    let all_points = izip!(
        all_points.iter_indices(),
        all_points.iter_dense::<MyPoint>(&resolver)
    );
    let all_colors = izip!(
        all_colors.iter_indices(),
        all_colors.iter_sparse::<MyColor>(&resolver)
    );

    let mut results = re_query2::range_zip_1x1(all_points, all_colors);

    {
        // Frame #323

        let ((data_time, _row_id), points, colors) = results.next().unwrap();
        assert_eq!(TimeInt::new_temporal(323), data_time);

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

    let all_points = results.get_required(MyPoint::name())?;
    let all_colors = results.get_or_empty(MyColor::name());

    let all_points = izip!(
        all_points.iter_indices(),
        all_points.iter_dense::<MyPoint>(&resolver)
    );
    let all_colors = izip!(
        all_colors.iter_indices(),
        all_colors.iter_sparse::<MyColor>(&resolver)
    );

    let mut results = re_query2::range_zip_1x1(all_points, all_colors);

    {
        // Frame #123 (partially static)

        let ((data_time, _row_id), points, colors) = results.next().unwrap();
        assert_eq!(TimeInt::new_temporal(123), data_time);

        let expected_points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
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
    {
        // Frame #323

        let ((data_time, _row_id), points, colors) = results.next().unwrap();
        assert_eq!(TimeInt::new_temporal(323), data_time);

        let expected_points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let expected_colors = vec![
            Some(MyColor::from_rgb(255, 0, 0)),
            Some(MyColor::from_rgb(255, 0, 0)),
        ];

        let points = points.flatten().unwrap();
        let colors = colors.map_or(vec![], |colors| colors.flatten().unwrap());

        let (got_points, got_colors): (Vec<_>, Vec<_>) =
            re_query2::clamped_zip_1x1(points, colors, || None).unzip();

        eprintln!("{}", store.to_data_table().unwrap());

        similar_asserts::assert_eq!(expected_points, got_points);
        similar_asserts::assert_eq!(expected_colors, got_colors);
    }

    // --- Third test: `[-inf, +inf]` ---

    let query =
        re_data_store::RangeQuery::new(timepoint1[0].0, TimeRange::new(TimeInt::MIN, TimeInt::MAX));

    let results = re_query2::range(
        &store,
        &query,
        &entity_path,
        MyPoints::all_components().iter().copied(),
    );

    let all_points = results.get_required(MyPoint::name())?;
    let all_colors = results.get_or_empty(MyColor::name());

    let all_points = izip!(
        all_points.iter_indices(),
        all_points.iter_dense::<MyPoint>(&resolver)
    );
    let all_colors = izip!(
        all_colors.iter_indices(),
        all_colors.iter_sparse::<MyColor>(&resolver)
    );

    let mut results = re_query2::range_zip_1x1(all_points, all_colors);

    {
        // Frame #123 (partially static)

        let ((data_time, _row_id), points, colors) = results.next().unwrap();
        assert_eq!(TimeInt::new_temporal(123), data_time);

        let expected_points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
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
    {
        // Frame #323

        let ((data_time, _row_id), points, colors) = results.next().unwrap();
        assert_eq!(TimeInt::new_temporal(323), data_time);

        let expected_points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let expected_colors = vec![
            Some(MyColor::from_rgb(255, 0, 0)),
            Some(MyColor::from_rgb(255, 0, 0)),
        ];

        let points = points.flatten().unwrap();
        let colors = colors.map_or(vec![], |colors| colors.flatten().unwrap());

        let (got_points, got_colors): (Vec<_>, Vec<_>) =
            re_query2::clamped_zip_1x1(points, colors, || None).unzip();

        eprintln!("{}", store.to_data_table().unwrap());

        similar_asserts::assert_eq!(expected_points, got_points);
        similar_asserts::assert_eq!(expected_colors, got_colors);
    }

    Ok(())
}
