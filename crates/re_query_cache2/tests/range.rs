use itertools::{izip, Itertools as _};

use re_data_store::{DataStore, RangeQuery, StoreSubscriber as _, TimeInt, TimeRange};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyPoint, MyPoints},
    DataRow, EntityPath, RowId, TimePoint,
};
use re_query_cache2::{Caches, PromiseResolver, PromiseResult};
use re_types::{components::InstanceKey, Archetype};
use re_types_core::Loggable as _;

// ---

#[test]
fn simple_range() -> anyhow::Result<()> {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123)];
    {
        let points = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 2, points)?;
        insert_and_react(&mut store, &mut caches, &row);

        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 1, colors)?;
        insert_and_react(&mut store, &mut caches, &row);
    }

    let timepoint2 = [build_frame_nr(223)];
    {
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint2, 1, colors)?;
        insert_and_react(&mut store, &mut caches, &row);
    }

    let timepoint3 = [build_frame_nr(323)];
    {
        let points = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint3, 2, points)?;
        insert_and_react(&mut store, &mut caches, &row);
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1.as_i64() + 1, timepoint3[0].1),
    );

    query_and_compare(&caches, &store, &query, &entity_path);

    // --- Second test: `[timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    query_and_compare(&caches, &store, &query, &entity_path);

    Ok(())
}

#[test]
fn static_range() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123)];
    {
        let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 2, positions)
                .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path.clone(),
            timepoint1,
            1,
            colors.clone(),
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        // Insert statically too!
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path.clone(),
            TimePoint::default(),
            1,
            colors,
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);
    }

    let timepoint2 = [build_frame_nr(223)];
    {
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path.clone(),
            timepoint2,
            1,
            colors.clone(),
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        // Insert statically too!
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path.clone(),
            TimePoint::default(),
            1,
            colors,
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);
    }

    let timepoint3 = [build_frame_nr(323)];
    {
        // Create some Positions with implicit instances
        let positions = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint3, 2, positions)
                .unwrap();
        insert_and_react(&mut store, &mut caches, &row);
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1.as_i64() + 1, timepoint3[0].1),
    );

    query_and_compare(&caches, &store, &query, &entity_path);

    // --- Second test: `[timepoint1, timepoint3]` ---

    // The inclusion of `timepoint1` means latest-at semantics will fall back to timeless data!

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    query_and_compare(&caches, &store, &query, &entity_path);

    // --- Third test: `[-inf, +inf]` ---

    let query =
        re_data_store::RangeQuery::new(timepoint1[0].0, TimeRange::new(TimeInt::MIN, TimeInt::MAX));

    query_and_compare(&caches, &store, &query, &entity_path);
}

#[test]
fn simple_splatted_range() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path: EntityPath = "point".into();

    let timepoint1 = [build_frame_nr(123)];
    {
        let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 2, positions)
                .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        // Assign one of them a color with an explicit instance
        let colors = vec![MyColor::from_rgb(255, 0, 0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint1, 1, colors)
                .unwrap();
        insert_and_react(&mut store, &mut caches, &row);
    }

    let timepoint2 = [build_frame_nr(223)];
    {
        let colors = vec![MyColor::from_rgb(0, 255, 0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint2, 1, colors)
                .unwrap();
        insert_and_react(&mut store, &mut caches, &row);
    }

    let timepoint3 = [build_frame_nr(323)];
    {
        let positions = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path.clone(), timepoint3, 2, positions)
                .unwrap();
        insert_and_react(&mut store, &mut caches, &row);
    }

    // --- First test: `(timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1.as_i64() + 1, timepoint3[0].1),
    );

    query_and_compare(&caches, &store, &query, &entity_path);

    // --- Second test: `[timepoint1, timepoint3]` ---

    let query = re_data_store::RangeQuery::new(
        timepoint1[0].0,
        TimeRange::new(timepoint1[0].1, timepoint3[0].1),
    );

    query_and_compare(&caches, &store, &query, &entity_path);
}

#[test]
fn invalidation() {
    let entity_path = "point";

    let test_invalidation = |query: RangeQuery,
                             present_data_timepoint: TimePoint,
                             past_data_timepoint: TimePoint,
                             future_data_timepoint: TimePoint| {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            Default::default(),
        );
        let mut caches = Caches::new(&store);

        let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            present_data_timepoint.clone(),
            2,
            positions,
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        let colors = vec![MyColor::from_rgb(1, 2, 3)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            present_data_timepoint.clone(),
            1,
            colors,
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        query_and_compare(&caches, &store, &query, &entity_path.into());

        // --- Modify present ---

        // Modify the PoV component
        let positions = vec![MyPoint::new(10.0, 20.0), MyPoint::new(30.0, 40.0)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            present_data_timepoint.clone(),
            2,
            positions,
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        query_and_compare(&caches, &store, &query, &entity_path.into());

        // Modify the optional component
        let colors = vec![MyColor::from_rgb(4, 5, 6), MyColor::from_rgb(7, 8, 9)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            present_data_timepoint,
            2,
            colors,
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        query_and_compare(&caches, &store, &query, &entity_path.into());

        // --- Modify past ---

        // Modify the PoV component
        let positions = vec![MyPoint::new(100.0, 200.0), MyPoint::new(300.0, 400.0)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            past_data_timepoint.clone(),
            2,
            positions,
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        query_and_compare(&caches, &store, &query, &entity_path.into());

        // Modify the optional component
        let colors = vec![MyColor::from_rgb(10, 11, 12), MyColor::from_rgb(13, 14, 15)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            past_data_timepoint.clone(),
            2,
            colors,
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        query_and_compare(&caches, &store, &query, &entity_path.into());

        // --- Modify future ---

        // Modify the PoV component
        let positions = vec![MyPoint::new(1000.0, 2000.0), MyPoint::new(3000.0, 4000.0)];
        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path,
            future_data_timepoint.clone(),
            2,
            positions,
        )
        .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        query_and_compare(&caches, &store, &query, &entity_path.into());

        // Modify the optional component
        let colors = vec![MyColor::from_rgb(16, 17, 18)];
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path, future_data_timepoint, 1, colors)
                .unwrap();
        insert_and_react(&mut store, &mut caches, &row);

        query_and_compare(&caches, &store, &query, &entity_path.into());
    };

    let timeless = TimePoint::default();
    let frame_122 = build_frame_nr(122);
    let frame_123 = build_frame_nr(123);
    let frame_124 = build_frame_nr(124);

    test_invalidation(
        RangeQuery::new(frame_123.0, TimeRange::EVERYTHING),
        [frame_123].into(),
        [frame_122].into(),
        [frame_124].into(),
    );

    test_invalidation(
        RangeQuery::new(frame_123.0, TimeRange::EVERYTHING),
        [frame_123].into(),
        timeless,
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
    // TODO(cmc): this test is coming back in the next PR.
    if true {
        return;
    }

    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path = "points";

    let timeless = TimePoint::default();
    let frame2 = [build_frame_nr(2)];
    let frame3 = [build_frame_nr(3)];

    let query = re_data_store::RangeQuery::new(frame2[0].0, TimeRange::EVERYTHING);

    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timeless, 2, positions).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    query_and_compare(&caches, &store, &query, &entity_path.into());

    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, frame2, 1, colors).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    query_and_compare(&caches, &store, &query, &entity_path.into());

    let colors = vec![MyColor::from_rgb(0, 0, 255)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, frame3, 1, colors).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    query_and_compare(&caches, &store, &query, &entity_path.into());

    let colors = vec![MyColor::from_rgb(0, 255, 0)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, frame3, 1, colors).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    query_and_compare(&caches, &store, &query, &entity_path.into());
}

#[test]
fn invalidation_static() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path = "points";

    let timeless = TimePoint::default();

    let frame0 = [build_frame_nr(TimeInt::ZERO)];
    let query = re_data_store::RangeQuery::new(frame0[0].0, TimeRange::EVERYTHING);

    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timeless.clone(), 2, positions)
        .unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    query_and_compare(&caches, &store, &query, &entity_path.into());

    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timeless.clone(), 1, colors).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    query_and_compare(&caches, &store, &query, &entity_path.into());

    let colors = vec![MyColor::from_rgb(0, 0, 255)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timeless, 1, colors).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    query_and_compare(&caches, &store, &query, &entity_path.into());
}

// ---

fn insert_and_react(store: &mut DataStore, caches: &mut Caches, row: &DataRow) {
    caches.on_events(&[store.insert_row(row).unwrap()]);
}

fn query_and_compare(
    caches: &Caches,
    store: &DataStore,
    query: &RangeQuery,
    entity_path: &EntityPath,
) {
    re_log::setup_logging();

    let resolver = PromiseResolver::default();

    for _ in 0..3 {
        let cached = caches.range(
            store,
            query,
            entity_path,
            MyPoints::all_components().iter().copied(),
        );

        let cached_all_points = cached
            .get_required(MyPoint::name())
            .unwrap()
            .to_dense::<MyPoint>(&resolver);
        assert!(matches!(
            cached_all_points.status(query.range()),
            (PromiseResult::Ready(()), PromiseResult::Ready(())),
        ));
        let cached_all_points_indexed = cached_all_points.range_indexed(query.range());

        let cached_all_colors = cached
            .get_or_empty(MyColor::name())
            .to_sparse::<MyColor>(&resolver);
        assert!(matches!(
            cached_all_colors.status(query.range()),
            (PromiseResult::Ready(()), PromiseResult::Ready(())),
        ));
        let cached_all_colors_indexed = cached_all_colors.range_indexed(query.range());

        let expected = re_query2::range(
            store,
            query,
            entity_path,
            MyPoints::all_components().iter().copied(),
        );

        let expected_all_points = expected.get_required(MyPoint::name()).unwrap();
        let expected_all_points_indices = expected_all_points.indices();
        let expected_all_points_data = expected_all_points
            .to_dense::<MyPoint>(&resolver)
            .into_iter()
            .map(|batch| batch.flatten().unwrap())
            .collect_vec();
        let expected_all_points_indexed =
            izip!(expected_all_points_indices, expected_all_points_data);

        let expected_all_colors = expected.get_or_empty(MyColor::name());
        let expected_all_colors_indices = expected_all_colors.indices();
        let expected_all_colors_data = expected_all_colors
            .to_sparse::<MyColor>(&resolver)
            .into_iter()
            .map(|batch| batch.flatten().unwrap())
            .collect_vec();
        let expected_all_colors_indexed =
            izip!(expected_all_colors_indices, expected_all_colors_data);

        eprintln!("{query:?}");
        eprintln!("{}", store.to_data_table().unwrap());

        similar_asserts::assert_eq!(
            expected_all_points_indexed
                .map(|(index, data)| (*index, data))
                .collect_vec(),
            cached_all_points_indexed
                .map(|(index, data)| (*index, data.to_vec()))
                .collect_vec(),
        );

        similar_asserts::assert_eq!(
            expected_all_colors_indexed
                .map(|(index, data)| (*index, data))
                .collect_vec(),
            cached_all_colors_indexed
                .map(|(index, data)| (*index, data.to_vec()))
                .collect_vec(),
        );
    }
}
