//! Contains:
//! - A 1:1 port of the tests in `crates/re_query/tests/archetype_query_tests.rs`, with caching enabled.
//! - Invalidation tests.

use re_data_store::{DataStore, LatestAtQuery, StoreSubscriber};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyPoint, MyPoints},
    DataRow, EntityPath, RowId, TimePoint,
};
use re_query2::PromiseResolver;
use re_query_cache2::Caches;
use re_types::Archetype as _;
use re_types_core::{components::InstanceKey, Loggable as _};

// ---

#[test]
fn simple_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some positions with implicit instances
    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, positions).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    // Assign one of them a color with an explicit instance
    let color_instances = vec![InstanceKey(1)];
    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        entity_path,
        timepoint,
        1,
        (color_instances, colors),
    )
    .unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());
}

#[test]
fn static_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some positions with implicit instances
    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, positions).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    // Assign one of them a color with an explicit instance.. statically!
    let color_instances = vec![InstanceKey(1)];
    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        entity_path,
        TimePoint::default(),
        1,
        (color_instances, colors),
    )
    .unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());
}

#[test]
fn no_instance_join_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some positions with an implicit instance
    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, positions).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    // Assign them colors with explicit instances
    let colors = vec![MyColor::from_rgb(255, 0, 0), MyColor::from_rgb(0, 255, 0)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, colors).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());
}

#[test]
fn missing_column_join_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some positions with an implicit instance
    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, positions).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());
}

#[test]
fn splatted_query() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path = "point";
    let timepoint = [build_frame_nr(123)];

    // Create some positions with implicit instances
    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timepoint, 2, positions).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    // Assign all of them a color via splat
    let color_instances = vec![InstanceKey::SPLAT];
    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        entity_path,
        timepoint,
        1,
        (color_instances, colors),
    )
    .unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(timepoint[0].0, timepoint[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());
}

#[test]
fn invalidation_xxx() {
    let entity_path = "point";

    let test_invalidation = |query: LatestAtQuery,
                             present_data_timepoint: TimePoint,
                             past_data_timepoint: TimePoint,
                             future_data_timepoint: TimePoint| {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            InstanceKey::name(),
            Default::default(),
        );
        let mut caches = Caches::new(&store);

        // Create some positions with implicit instances
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

        // Assign one of them a color with an explicit instance
        let color_instances = vec![InstanceKey(1)];
        let colors = vec![MyColor::from_rgb(1, 2, 3)];
        let row = DataRow::from_cells2_sized(
            RowId::new(),
            entity_path,
            present_data_timepoint.clone(),
            1,
            (color_instances, colors),
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
        let row =
            DataRow::from_cells1_sized(RowId::new(), entity_path, past_data_timepoint, 2, colors)
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
        LatestAtQuery::new(frame_123.0, frame_123.1),
        [frame_123].into(),
        [frame_122].into(),
        [frame_124].into(),
    );

    test_invalidation(
        LatestAtQuery::new(frame_123.0, frame_123.1),
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

    let query_time = [build_frame_nr(9999)];

    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row =
        DataRow::from_cells1_sized(RowId::new(), entity_path, timeless, 2, positions).unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());

    let color_instances = vec![InstanceKey::SPLAT];
    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        entity_path,
        frame2,
        1,
        (color_instances, colors),
    )
    .unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());

    let color_instances = vec![InstanceKey::SPLAT];
    let colors = vec![MyColor::from_rgb(0, 0, 255)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        entity_path,
        frame3,
        1,
        (color_instances, colors),
    )
    .unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());

    let color_instances = vec![InstanceKey::SPLAT];
    let colors = vec![MyColor::from_rgb(0, 255, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        entity_path,
        frame3,
        1,
        (color_instances, colors),
    )
    .unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());
}

#[test]
fn static_invalidation() {
    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        InstanceKey::name(),
        Default::default(),
    );
    let mut caches = Caches::new(&store);

    let entity_path = "points";

    let timeless = TimePoint::default();

    let query_time = [build_frame_nr(9999)];

    let positions = vec![MyPoint::new(1.0, 2.0), MyPoint::new(3.0, 4.0)];
    let row = DataRow::from_cells1_sized(RowId::new(), entity_path, timeless.clone(), 2, positions)
        .unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());

    let color_instances = vec![InstanceKey::SPLAT];
    let colors = vec![MyColor::from_rgb(255, 0, 0)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        entity_path,
        timeless.clone(),
        1,
        (color_instances, colors),
    )
    .unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());

    let color_instances = vec![InstanceKey::SPLAT];
    let colors = vec![MyColor::from_rgb(0, 0, 255)];
    let row = DataRow::from_cells2_sized(
        RowId::new(),
        entity_path,
        timeless,
        1,
        (color_instances, colors),
    )
    .unwrap();
    insert_and_react(&mut store, &mut caches, &row);

    let query = re_data_store::LatestAtQuery::new(query_time[0].0, query_time[0].1);
    query_and_compare(&caches, &store, &query, &entity_path.into());
}

// ---

fn insert_and_react(store: &mut DataStore, caches: &mut Caches, row: &DataRow) {
    caches.on_events(&[store.insert_row(row).unwrap()]);
}

fn query_and_compare(
    caches: &Caches,
    store: &DataStore,
    query: &LatestAtQuery,
    entity_path: &EntityPath,
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
        let cached_point_data = cached_points
            .to_dense::<MyPoint>(&resolver)
            .flatten()
            .unwrap();

        let cached_colors = cached.get_or_empty(MyColor::name());
        let cached_color_data = cached_colors
            .to_dense::<MyColor>(&resolver)
            .flatten()
            .unwrap();

        let expected = re_query2::latest_at(
            store,
            query,
            entity_path,
            MyPoints::all_components().iter().copied(),
        );

        let expected_points = expected.get_required(MyPoint::name()).unwrap();
        let expected_point_data = expected_points
            .to_dense::<MyPoint>(&resolver)
            .flatten()
            .unwrap();

        let expected_colors = expected.get_or_empty(MyColor::name());
        let expected_color_data = expected_colors
            .to_dense::<MyColor>(&resolver)
            .flatten()
            .unwrap();

        // eprintln!("{}", store.to_data_table().unwrap());

        similar_asserts::assert_eq!(expected.compound_index, cached.compound_index);
        similar_asserts::assert_eq!(expected_point_data, cached_point_data);
        similar_asserts::assert_eq!(expected_color_data, cached_color_data);
    }
}
