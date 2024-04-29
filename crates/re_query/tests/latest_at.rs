use re_data_store::{DataStore, LatestAtQuery, StoreSubscriber};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyPoint, MyPoints},
    DataRow, EntityPath, RowId, TimeInt, TimePoint,
};
use re_query::PromiseResolver;
use re_query::{Caches, DataStoreRef};
use re_types::Archetype as _;
use re_types_core::Loggable as _;

// ---

#[test]
fn simple_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );

    let mut caches = Caches::new((&store).into());

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, points.clone()).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, colors.clone()).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    let expected_compound_index = (TimeInt::new_temporal(123), row.row_id());
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        (&store).into(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );
}

#[test]
fn static_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = Caches::new((&store).into());

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row1 =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, points.clone()).unwrap();
    insert_and_react(&mut store, &mut caches, &row1);

    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row2 = DataRow::from_cells1_sized(
        RowId::new(),
        entity_path,
        TimePoint::default(),
        colors.clone(),
    )
    .unwrap();
    insert_and_react(&mut store, &mut caches, &row2);

    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    let expected_compound_index = (TimeInt::new_temporal(123), row1.row_id());
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        (&store).into(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );
}

#[test]
fn invalidation() {
    let entity_path = "point";

    let test_invalidation = |query: LatestAtQuery,
                             present_data_timepoint: TimePoint,
                             past_data_timepoint: TimePoint,
                             future_data_timepoint: TimePoint| {
        let past_timestamp = past_data_timepoint
            .get(&query.timeline())
            .copied()
            .unwrap_or(TimeInt::STATIC);
        let present_timestamp = present_data_timepoint
            .get(&query.timeline())
            .copied()
            .unwrap_or(TimeInt::STATIC);

        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );
        let mut caches = Caches::new((&store).into());

        let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row1 = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            present_data_timepoint.clone(),
            points.clone(),
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row1);

        let colors = vec![MyColor::from_rgb(1, 2, 3)];
        let row2 = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            present_data_timepoint.clone(),
            colors.clone(),
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row2);

        let expected_compound_index = (present_timestamp, row2.row_id());
        let expected_points = &points;
        let expected_colors = &colors;
        query_and_compare(
            &caches,
            (&store).into(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // --- Modify present ---

        // Modify the PoV component
        let points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row3 = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            present_data_timepoint.clone(),
            points.clone(),
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row3);

        let expected_compound_index = (present_timestamp, row3.row_id());
        let expected_points = &points;
        let expected_colors = &colors;
        query_and_compare(
            &caches,
            (&store).into(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // Modify the optional component
        let colors = vec![MyColor::from_rgb(4, 5, 6), MyColor::from_rgb(7, 8, 9)];
        let row4 = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            present_data_timepoint.clone(),
            colors.clone(),
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row4);

        let expected_compound_index = (present_timestamp, row4.row_id());
        let expected_points = &points;
        let expected_colors = &colors;
        query_and_compare(
            &caches,
            (&store).into(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // --- Modify past ---

        // Modify the PoV component
        let points_past = vec![MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)];
        let row5 = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            past_data_timepoint.clone(),
            points_past.clone(),
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row5);

        let expected_compound_index = (present_timestamp, row4.row_id());
        let expected_points = if past_timestamp.is_static() {
            &points_past
        } else {
            &points
        };
        let expected_colors = &colors;
        query_and_compare(
            &caches,
            (&store).into(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // Modify the optional component
        let colors_past = vec![MyColor::from_rgb(10, 11, 12), MyColor::from_rgb(13, 14, 15)];
        let row6 = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            past_data_timepoint,
            colors_past.clone(),
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row6);

        let (expected_compound_index, expected_colors) = if past_timestamp.is_static() {
            ((past_timestamp, row6.row_id()), &colors_past)
        } else {
            ((present_timestamp, row4.row_id()), &colors)
        };
        query_and_compare(
            &caches,
            (&store).into(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // --- Modify future ---

        // Modify the PoV component
        let points_future = vec![MyPoint::new(1000.0, 2000.0), MyPoint::new(3000.0, 4000.0)];
        let row7 = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            future_data_timepoint.clone(),
            points_future.clone(),
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row7);

        let (expected_compound_index, expected_points) = if past_timestamp.is_static() {
            ((past_timestamp, row6.row_id()), &points_past)
        } else {
            ((present_timestamp, row4.row_id()), &points)
        };
        query_and_compare(
            &caches,
            (&store).into(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );

        // Modify the optional component
        let colors_future = vec![MyColor::from_rgb(16, 17, 18)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            future_data_timepoint,
            colors_future,
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        let (expected_compound_index, expected_colors) = if past_timestamp.is_static() {
            ((past_timestamp, row6.row_id()), &colors_past)
        } else {
            ((present_timestamp, row4.row_id()), &colors)
        };
        query_and_compare(
            &caches,
            (&store).into(),
            &query,
            &entity_path.into(),
            expected_compound_index,
            expected_points,
            expected_colors,
        );
    };

    let static_ = TimePoint::default();
    let frame_122 = build_frame_nr(122);
    let frame_123 = build_frame_nr(123);
    let frame_124 = build_frame_nr(124);

    test_invalidation(
        LatestAtQuery::new(frame_123.0, frame_123.1),
        [frame_123].into(),
        [frame_122].into(),
        [frame_124].into(),
    );

    test_invalidation(
        LatestAtQuery::new(frame_123.0, frame_123.1),
        [frame_123].into(),
        static_,
        [frame_124].into(),
    );
}

// Test the following scenario:
// ```py
// rr.log("points", rr.Points3D([1, 2, 3]), static=True)
//
// # Do first query here: LatestAt(+inf)
// # Expected: points=[[1,2,3]] colors=[]
//
// rr.set_time(2)
// rr.log_components("points", rr.components.MyColor(0xFF0000))
//
// # Do second query here: LatestAt(+inf)
// # Expected: points=[[1,2,3]] colors=[0xFF0000]
//
// rr.set_time(3)
// rr.log_components("points", rr.components.MyColor(0x0000FF))
//
// # Do third query here: LatestAt(+inf)
// # Expected: points=[[1,2,3]] colors=[0x0000FF]
//
// rr.set_time(3)
// rr.log_components("points", rr.components.MyColor(0x00FF00))
//
// # Do fourth query here: LatestAt(+inf)
// # Expected: points=[[1,2,3]] colors=[0x00FF00]
// ```
#[test]
fn invalidation_of_future_optionals() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = Caches::new((&store).into());

    let entity_path = "points";

    let static_ = TimePoint::default();
    let frame2 = [build_frame_nr(2)];
    let frame3 = [build_frame_nr(3)];

    let query_time = [build_frame_nr(9999)];

    let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row1 =
        DataRow::from_cells1_sized(RowId::new(), entity_path, static_, points.clone()).unwrap();
    insert_and_react(&mut store, &mut caches, &row1);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::STATIC, row1.row_id());
    let expected_points = &points;
    let expected_colors = &[];
    query_and_compare(
        &caches,
        (&store).into(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );

    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row2 =
        DataRow::from_cells1_sized(RowId::new(), entity_path, frame2, colors.clone()).unwrap();
    insert_and_react(&mut store, &mut caches, &row2);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::new_temporal(2), row2.row_id());
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        (&store).into(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );

    let colors = vec![MyColor::from_rgb(0, 0, 255)];
    let row3 =
        DataRow::from_cells1_sized(RowId::new(), entity_path, frame3, colors.clone()).unwrap();
    insert_and_react(&mut store, &mut caches, &row3);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::new_temporal(3), row3.row_id());
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        (&store).into(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );

    let colors = vec![MyColor::from_rgb(0, 255, 0)];
    let row4 =
        DataRow::from_cells1_sized(RowId::new(), entity_path, frame3, colors.clone()).unwrap();
    insert_and_react(&mut store, &mut caches, &row4);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::new_temporal(3), row4.row_id());
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        (&store).into(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );
}

#[test]
fn static_invalidation() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );
    let mut caches = Caches::new((&store).into());

    let entity_path = "points";

    let timeless = TimePoint::default();

    let query_time = [build_frame_nr(9999)];

    let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row1 =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timeless.clone(), points.clone())
            .unwrap();
    insert_and_react(&mut store, &mut caches, &row1);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::STATIC, row1.row_id());
    let expected_points = &points;
    let expected_colors = &[];
    query_and_compare(
        &caches,
        (&store).into(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );

    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row2 =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timeless.clone(), colors.clone())
            .unwrap();
    insert_and_react(&mut store, &mut caches, &row2);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::STATIC, row2.row_id());
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        (&store).into(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );

    let colors = vec![MyColor::from_rgb(0, 0, 255)];
    let row3 =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timeless, colors.clone()).unwrap();
    insert_and_react(&mut store, &mut caches, &row3);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    let expected_compound_index = (TimeInt::STATIC, row3.row_id());
    let expected_points = &points;
    let expected_colors = &colors;
    query_and_compare(
        &caches,
        (&store).into(),
        &query,
        &entity_path.into(),
        expected_compound_index,
        expected_points,
        expected_colors,
    );
}

// ---

fn insert_and_react(store: &mut DataStore, caches: &mut Caches, row: &DataRow) {
    caches.on_events(&[store.insert_row(row).unwrap()]);
}

fn query_and_compare(
    caches: &Caches,
    store: DataStoreRef<'_>,
    query: &LatestAtQuery,
    entity_path: &EntityPath,
    expected_compound_index: (TimeInt, RowId),
    expected_points: &[MyPoint],
    expected_colors: &[MyColor],
) {
    re_log::setup_logging();

    let resolver = PromiseResolver::default();

    for _ in 0..3 {
        let cached = caches.latest_at(
            store,
            query,
            entity_path,
            MyPoints::all_components().iter().copied(),
        );

        let cached_points = cached.get_required(MyPoint::name()).unwrap();
        let cached_points = cached_points
            .to_dense::<MyPoint>(&resolver)
            .flatten()
            .unwrap();

        let cached_colors = cached.get_or_empty(MyColor::name());
        let cached_colors = cached_colors
            .to_dense::<MyColor>(&resolver)
            .flatten()
            .unwrap();

        // eprintln!("{}", store.to_data_table().unwrap());

        similar_asserts::assert_eq!(expected_compound_index, cached.compound_index);
        similar_asserts::assert_eq!(expected_points, cached_points);
        similar_asserts::assert_eq!(expected_colors, cached_colors);
    }
}
