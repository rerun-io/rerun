use re_data_store::DataStore;
use re_log_types::example_components::{MyColor, MyPoint, MyPoints};
use re_log_types::{build_frame_nr, DataRow, RowId, TimePoint};
use re_query2::PromiseResolver;
use re_types::components::InstanceKey;
use re_types::{Archetype as _, ComponentNameSet};
use re_types_core::Loggable as _;

// ---

#[test]
fn simple_query() -> anyhow::Result<()> {
    let mut resolver = PromiseResolver::default();

    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, points)?;
    store.insert_row(&row)?;

    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 1, colors)?;
    store.insert_row(&row)?;

    let timeline_query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    let results = re_query2::latest_at(
        &store,
        &timeline_query,
        &entity_path.into(),
        MyPoints::all_components().iter().cloned(),
    );

    // We expect this to generate the following `DataFrame`
    // ┌─────────────┬────────────┐
    // │ point       ┆ color      │
    // │ ---         ┆ ---        │
    // │ struct[2]   ┆ u32        │
    // ╞═════════════╪════════════╡
    // │ {1.0,2.0}   ┆ 4278190080 │
    // ├╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {3.0,4.0}   ┆ 4278190080 │
    // └─────────────┴────────────┘

    {
        let expected_components: ComponentNameSet =
            [MyPoint::name(), MyColor::name()].into_iter().collect();
        let got_components: ComponentNameSet = results.components.keys().copied().collect();
        similar_asserts::assert_eq!(expected_components, got_components);

        let expected_points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let expected_colors = vec![
            Some(MyColor::from_rgb(255, 0, 0)),
            Some(MyColor::from_rgb(255, 0, 0)),
        ];

        let points = results.get_required::<MyPoint>()?;
        let point_data = points
            .iter_dense::<MyPoint>(&mut resolver)
            .flatten()
            .unwrap();

        let colors = results.get_optional::<MyColor>();
        let color_data = colors
            .iter_sparse::<MyColor>(&mut resolver)
            .flatten()
            .unwrap();
        let color_default_fn = || Some(MyColor::from(0xFF00FFFF));

        let (got_points, got_colors): (Vec<_>, Vec<_>) =
            re_query2::clamped_zip_1x1(point_data, color_data, color_default_fn).unzip();

        similar_asserts::assert_eq!(expected_points, got_points);
        similar_asserts::assert_eq!(expected_colors, got_colors);
    }

    Ok(())
}

#[test]
fn static_query() -> anyhow::Result<()> {
    let mut resolver = PromiseResolver::default();

    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, positions)?;
    store.insert_row(&row)?;

    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, TimePoint::default(), 2, colors)?;
    store.insert_row(&row)?;

    let timeline_query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    let results = re_query2::latest_at(
        &store,
        &timeline_query,
        &entity_path.into(),
        MyPoints::all_components().iter().cloned(),
    );

    // We expect this to generate the following `DataFrame`
    // ┌───────────┬────────────┐
    // │ point     ┆ color      │
    // │ ---       ┆ ---        │
    // │ struct[2] ┆ u32        │
    // ╞═══════════╪════════════╡
    // │ {1.0,2.0} ┆ 4278190080 │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {3.0,4.0} ┆ 4278190080 │
    // └───────────┴────────────┘

    {
        let expected_components: ComponentNameSet =
            [MyPoint::name(), MyColor::name()].into_iter().collect();
        let got_components: ComponentNameSet = results.components.keys().copied().collect();
        similar_asserts::assert_eq!(expected_components, got_components);

        let expected_points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let expected_colors = vec![
            Some(MyColor::from_rgb(255, 0, 0)),
            Some(MyColor::from_rgb(255, 0, 0)),
        ];

        let points = results.get_required::<MyPoint>()?;
        let point_data = points
            .iter_dense::<MyPoint>(&mut resolver)
            .flatten()
            .unwrap();

        let colors = results.get_optional::<MyColor>();
        let color_data = colors
            .iter_sparse::<MyColor>(&mut resolver)
            .flatten()
            .unwrap();
        let color_default_fn = || Some(MyColor::from(0xFF00FFFF));

        let (got_points, got_colors): (Vec<_>, Vec<_>) =
            re_query2::clamped_zip_1x1(point_data, color_data, color_default_fn).unzip();

        similar_asserts::assert_eq!(expected_points, got_points);
        similar_asserts::assert_eq!(expected_colors, got_colors);
    }

    Ok(())
}

#[test]
fn no_instance_join_query() -> anyhow::Result<()> {
    let mut resolver = PromiseResolver::default();

    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, positions)?;
    store.insert_row(&row)?;

    let colors = vec![MyColor::from_rgb(255, 0, 0), MyColor::from_rgb(0, 255, 0)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, colors)?;
    store.insert_row(&row)?;

    let timeline_query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    let results = re_query2::latest_at(
        &store,
        &timeline_query,
        &entity_path.into(),
        MyPoints::all_components().iter().cloned(),
    );

    // We expect this to generate the following `DataFrame`
    // ┌───────────┬────────────┐
    // │ point     ┆ color      │
    // │ ---       ┆ ---        │
    // │ struct[2] ┆ u32        │
    // ╞═══════════╪════════════╡
    // │ {1.0,2.0} ┆ 4278190080 │
    // ├╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌┤
    // │ {3.0,4.0} ┆ 16711680   │
    // └───────────┴────────────┘

    {
        let expected_components: ComponentNameSet =
            [MyPoint::name(), MyColor::name()].into_iter().collect();
        let got_components: ComponentNameSet = results.components.keys().copied().collect();
        similar_asserts::assert_eq!(expected_components, got_components);

        let expected_points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let expected_colors = vec![
            Some(MyColor::from_rgb(255, 0, 0)),
            Some(MyColor::from_rgb(0, 255, 0)),
        ];

        let points = results.get_required::<MyPoint>()?;
        let point_data = points
            .iter_dense::<MyPoint>(&mut resolver)
            .flatten()
            .unwrap();

        let colors = results.get_optional::<MyColor>();
        let color_data = colors
            .iter_sparse::<MyColor>(&mut resolver)
            .flatten()
            .unwrap();
        let color_default_fn = || Some(MyColor::from(0xFF00FFFF));

        let (got_points, got_colors): (Vec<_>, Vec<_>) =
            re_query2::clamped_zip_1x1(point_data, color_data, color_default_fn).unzip();

        similar_asserts::assert_eq!(expected_points, got_points);
        similar_asserts::assert_eq!(expected_colors, got_colors);
    }

    Ok(())
}
