//! Correctness tests.
//!
//! Bending and twisting the datastore APIs in all kinds of weird ways to try and break them.

use rand::Rng;

use re_data_store::{
    test_row, test_util::sanity_unwrap, DataStore, DataStoreConfig, DataStoreStats,
    GarbageCollectionOptions, LatestAtQuery, WriteError,
};
use re_log_types::example_components::{MyColor, MyIndex, MyPoint};
use re_log_types::{
    build_frame_nr, build_log_time, DataRow, Duration, EntityPath, RowId, Time, TimeInt, TimePoint,
    TimeType, Timeline,
};
use re_types_core::Loggable as _;

// ---

fn query_latest_component<C: re_types_core::Component>(
    store: &DataStore,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> Option<(TimeInt, RowId, C)> {
    re_tracing::profile_function!();

    let (data_time, row_id, cells) =
        store.latest_at(query, entity_path, C::name(), &[C::name()])?;
    let cell = cells.first()?.as_ref()?;

    cell.try_to_native_mono::<C>()
        .ok()?
        .map(|c| (data_time, row_id, c))
}

// ---

#[test]
fn row_id_ordering_semantics() -> anyhow::Result<()> {
    let entity_path: EntityPath = "some_entity".into();

    let timeline_frame = Timeline::new_sequence("frame");
    let timepoint = TimePoint::from_iter([(timeline_frame, 10)]);

    let point1 = MyPoint::new(1.0, 1.0);
    let point2 = MyPoint::new(2.0, 2.0);

    // * Insert `point1` at frame #10 with a random `RowId`.
    // * Insert `point2` at frame #10 with a random `RowId`.
    // * Query at frame #11 and make sure we get `point2` because random `RowId`s are monotonically
    //   increasing.
    {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let row_id = RowId::new();
        let row = DataRow::from_component_batches(
            row_id,
            timepoint.clone(),
            entity_path.clone(),
            [&[point1] as _],
        )?;
        store.insert_row(&row)?;

        let row_id = RowId::new();
        let row = DataRow::from_component_batches(
            row_id,
            timepoint.clone(),
            entity_path.clone(),
            [&[point2] as _],
        )?;
        store.insert_row(&row)?;

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            let (_, _, got_point) =
                query_latest_component::<MyPoint>(&store, &entity_path, &query).unwrap();
            similar_asserts::assert_eq!(point2, got_point);
        }
    }

    // * Insert `point1` at frame #10 with a random `RowId`.
    // * Fail to insert `point2` at frame #10 using `point1`s `RowId` because it is illegal.
    {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let row_id = RowId::new();

        let row = DataRow::from_component_batches(
            row_id,
            timepoint.clone(),
            entity_path.clone(),
            [&[point1] as _],
        )?;
        store.insert_row(&row)?;

        let row = DataRow::from_component_batches(
            row_id,
            timepoint.clone(),
            entity_path.clone(),
            [&[point2] as _],
        )?;

        let res = store.insert_row(&row);
        assert!(matches!(res, Err(WriteError::ReusedRowId(_)),));
    }

    // * Insert `point1` at frame #10 with a random `RowId`.
    // * Insert `point2` at frame #10 using `point1`'s `RowId`, decremented by one.
    // * Query at frame #11 and make sure we get `point1` because of intra-timestamp tie-breaks.
    {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let row_id1 = RowId::new();
        let row_id2 = row_id1.next();

        let row = DataRow::from_component_batches(
            row_id2,
            timepoint.clone(),
            entity_path.clone(),
            [&[point1] as _],
        )?;
        store.insert_row(&row)?;

        let row = DataRow::from_component_batches(
            row_id1,
            timepoint.clone(),
            entity_path.clone(),
            [&[point2] as _],
        )?;
        store.insert_row(&row)?;

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            let (_, _, got_point) =
                query_latest_component::<MyPoint>(&store, &entity_path, &query).unwrap();
            similar_asserts::assert_eq!(point1, got_point);
        }
    }

    // Static data has last-write-wins semantics, as defined by RowId-ordering.
    // Timeless is RowId-ordered too!
    //
    // * Insert static `point1` with a random `RowId`.
    // * Insert static `point2` using `point1`'s `RowId`, decremented by one.
    // * Query and make sure we get `point1` because of last-write-wins semantics.
    {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let row_id1 = RowId::new();
        let row_id2 = row_id1.next();

        let row = DataRow::from_component_batches(
            row_id2,
            TimePoint::default(),
            entity_path.clone(),
            [&[point1] as _],
        )?;
        store.insert_row(&row)?;

        let row = DataRow::from_component_batches(
            row_id1,
            TimePoint::default(),
            entity_path.clone(),
            [&[point2] as _],
        )?;
        store.insert_row(&row)?;

        {
            let query = LatestAtQuery::new(Timeline::new_temporal("doesnt_matter"), TimeInt::MAX);
            let (_, _, got_point) =
                query_latest_component::<MyPoint>(&store, &entity_path, &query).unwrap();
            similar_asserts::assert_eq!(point1, got_point);
        }
    }

    Ok(())
}

// ---

#[test]
fn write_errors() {
    re_log::setup_logging();

    let entity_path = EntityPath::from("this/that");

    {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let mut row = test_row!(entity_path @ [
            build_frame_nr(1),
            build_log_time(Time::now()),
        ] => [ MyPoint::from_iter(0..1) ]);

        row.row_id = re_log_types::RowId::new();
        store.insert_row(&row).unwrap();

        row.row_id = row.row_id.next();
        store.insert_row(&row).unwrap();

        assert!(matches!(
            store.insert_row(&row),
            Err(WriteError::ReusedRowId(_)),
        ));

        let err = store.insert_row(&row).unwrap_err();
        let WriteError::ReusedRowId(err_row_id) = err else {
            unreachable!();
        };
        assert_eq!(row.row_id(), err_row_id);
    }
}

// ---

#[test]
fn latest_at_emptiness_edge_cases() {
    re_log::setup_logging();

    for config in re_data_store::test_util::all_configs() {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            config.clone(),
        );
        latest_at_emptiness_edge_cases_impl(&mut store);
    }
}

fn latest_at_emptiness_edge_cases_impl(store: &mut DataStore) {
    let entity_path = EntityPath::from("this/that");
    let now = Time::now();
    let now_minus_1s = now - Duration::from_secs(1.0);
    let now_minus_1s_nanos = now_minus_1s.nanos_since_epoch();
    let frame39 = 39;
    let frame40 = 40;
    let num_instances = 3;

    store
        .insert_row(&test_row!(entity_path @ [
                build_log_time(now), build_frame_nr(frame40),
            ] => [MyIndex::from_iter(0..num_instances as _)]))
        .unwrap();

    sanity_unwrap(store);

    let timeline_wrong_name = Timeline::new("lag_time", TimeType::Time);
    let timeline_wrong_kind = Timeline::new("log_time", TimeType::Sequence);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_log_time = Timeline::log_time();

    // empty frame_nr
    {
        let cells = store.latest_at(
            &LatestAtQuery::new(timeline_frame_nr, frame39),
            &entity_path,
            MyIndex::name(),
            &[MyIndex::name()],
        );
        assert!(cells.is_none());
    }

    // empty log_time
    {
        let cells = store.latest_at(
            &LatestAtQuery::new(timeline_log_time, now_minus_1s_nanos),
            &entity_path,
            MyIndex::name(),
            &[MyIndex::name()],
        );
        assert!(cells.is_none());
    }

    // wrong entity path
    {
        let cells = store.latest_at(
            &LatestAtQuery::new(timeline_frame_nr, frame40),
            &EntityPath::from("does/not/exist"),
            MyIndex::name(),
            &[MyIndex::name()],
        );
        assert!(cells.is_none());
    }

    // bunch of non-existing components
    {
        let components = &["does".into(), "not".into(), "exist".into()];
        let cells = store.latest_at(
            &LatestAtQuery::new(timeline_frame_nr, frame40),
            &entity_path,
            MyIndex::name(),
            components,
        );
        assert!(cells.is_none());
    }

    // empty component list
    {
        let cells = store.latest_at(
            &LatestAtQuery::new(timeline_frame_nr, frame40),
            &entity_path,
            MyIndex::name(),
            &[],
        );
        assert!(cells.is_none());
    }

    // wrong timeline name
    {
        let cells = store.latest_at(
            &LatestAtQuery::new(timeline_wrong_name, frame40),
            &EntityPath::from("does/not/exist"),
            MyIndex::name(),
            &[MyIndex::name()],
        );
        assert!(cells.is_none());
    }

    // wrong timeline kind
    {
        let cells = store.latest_at(
            &LatestAtQuery::new(timeline_wrong_kind, frame40),
            &EntityPath::from("does/not/exist"),
            MyIndex::name(),
            &[MyIndex::name()],
        );
        assert!(cells.is_none());
    }
}

// ---

#[test]
fn gc_correct() {
    re_log::setup_logging();

    let mut store = DataStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        DataStoreConfig::default(),
    );

    let stats_empty = DataStoreStats::from_store(&store);

    let mut rng = rand::thread_rng();

    let num_frames = rng.gen_range(0..=100);
    let frames = (0..num_frames).filter(|_| rand::thread_rng().gen());
    for frame_nr in frames {
        let num_ents = 10;
        for i in 0..num_ents {
            let entity_path = EntityPath::from(format!("this/that/{i}"));
            let num_instances = rng.gen_range(0..=1_000);
            let row = test_row!(entity_path @ [
                build_frame_nr(frame_nr),
            ] => [
                MyColor::from_iter(0..num_instances),
            ]);
            store.insert_row(&row).unwrap();
        }
    }

    sanity_unwrap(&store);
    check_still_readable(&store);

    let stats = DataStoreStats::from_store(&store);

    let (store_events, stats_diff) = store.gc(&GarbageCollectionOptions::gc_everything());
    let stats_diff = stats_diff + stats_empty; // account for fixed overhead

    assert_eq!(
        stats.metadata_registry.num_rows,
        stats_diff.metadata_registry.num_rows
    );
    assert_eq!(
        stats.metadata_registry.num_bytes,
        stats_diff.metadata_registry.num_bytes
    );
    assert_eq!(stats.temporal.num_rows, stats_diff.temporal.num_rows);

    sanity_unwrap(&store);
    check_still_readable(&store);
    for event in store_events {
        assert!(store.row_metadata(&event.row_id).is_none());
    }

    let (store_events, stats_diff) = store.gc(&GarbageCollectionOptions::gc_everything());
    assert!(store_events.is_empty());
    assert_eq!(DataStoreStats::default(), stats_diff);

    sanity_unwrap(&store);
    check_still_readable(&store);
}

fn check_still_readable(store: &DataStore) {
    store.to_data_table().unwrap(); // simple way of checking that everything is still readable
}

// This used to panic because the GC will decrement the metadata_registry size trackers before
// getting the confirmation that the row was really removed.
#[test]
fn gc_metadata_size() -> anyhow::Result<()> {
    for enable_batching in [false, true] {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let point = MyPoint::new(1.0, 1.0);

        for _ in 0..3 {
            let row = DataRow::from_component_batches(
                RowId::new(),
                TimePoint::default(),
                "xxx".into(),
                [&[point] as _],
            )?;
            store.insert_row(&row).unwrap();
        }

        for _ in 0..2 {
            _ = store.gc(&GarbageCollectionOptions {
                target: re_data_store::GarbageCollectionTarget::DropAtLeastFraction(1.0),
                protect_latest: 1,
                purge_empty_tables: false,
                dont_protect: Default::default(),
                enable_batching,
                time_budget: std::time::Duration::MAX,
            });
            _ = store.gc(&GarbageCollectionOptions {
                target: re_data_store::GarbageCollectionTarget::DropAtLeastFraction(1.0),
                protect_latest: 1,
                purge_empty_tables: false,
                dont_protect: Default::default(),
                enable_batching,
                time_budget: std::time::Duration::MAX,
            });
        }
    }

    Ok(())
}

// ---

#[test]
fn entity_min_time_correct() -> anyhow::Result<()> {
    re_log::setup_logging();

    for config in re_data_store::test_util::all_configs() {
        let mut store = DataStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            config.clone(),
        );
        entity_min_time_correct_impl(&mut store)?;
    }

    Ok(())
}

fn entity_min_time_correct_impl(store: &mut DataStore) -> anyhow::Result<()> {
    let entity_path = EntityPath::from("this/that");
    let wrong_entity_path = EntityPath::from("this/that/other");

    let point = MyPoint::new(1.0, 1.0);
    let timeline_wrong_name = Timeline::new("lag_time", TimeType::Time);
    let timeline_wrong_kind = Timeline::new("log_time", TimeType::Sequence);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_log_time = Timeline::log_time();

    let now = Time::now();
    let now_plus_one = now + Duration::from_secs(1.0);
    let now_minus_one = now - Duration::from_secs(1.0);

    let row = DataRow::from_component_batches(
        RowId::new(),
        TimePoint::default()
            .with(timeline_log_time, now)
            .with(timeline_frame_nr, 42),
        entity_path.clone(),
        [&[point] as _],
    )?;

    store.insert_row(&row).unwrap();

    assert!(store
        .entity_min_time(&timeline_wrong_name, &entity_path)
        .is_none());
    assert!(store
        .entity_min_time(&timeline_wrong_kind, &entity_path)
        .is_none());
    assert_eq!(
        store.entity_min_time(&timeline_frame_nr, &entity_path),
        Some(TimeInt::new_temporal(42))
    );
    assert_eq!(
        store.entity_min_time(&timeline_log_time, &entity_path),
        Some(TimeInt::try_from(now).unwrap())
    );
    assert!(store
        .entity_min_time(&timeline_frame_nr, &wrong_entity_path)
        .is_none());

    // insert row in the future, these shouldn't be visible
    let row = DataRow::from_component_batches(
        RowId::new(),
        TimePoint::default()
            .with(timeline_log_time, now_plus_one)
            .with(timeline_frame_nr, 54),
        entity_path.clone(),
        [&[point] as _],
    )?;
    store.insert_row(&row).unwrap();

    assert!(store
        .entity_min_time(&timeline_wrong_name, &entity_path)
        .is_none());
    assert!(store
        .entity_min_time(&timeline_wrong_kind, &entity_path)
        .is_none());
    assert_eq!(
        store.entity_min_time(&timeline_frame_nr, &entity_path),
        Some(TimeInt::new_temporal(42))
    );
    assert_eq!(
        store.entity_min_time(&timeline_log_time, &entity_path),
        Some(TimeInt::try_from(now).unwrap())
    );
    assert!(store
        .entity_min_time(&timeline_frame_nr, &wrong_entity_path)
        .is_none());

    // insert row in the past, these should be visible
    let row = DataRow::from_component_batches(
        RowId::new(),
        TimePoint::default()
            .with(timeline_log_time, now_minus_one)
            .with(timeline_frame_nr, 32),
        entity_path.clone(),
        [&[point] as _],
    )?;
    store.insert_row(&row).unwrap();

    assert!(store
        .entity_min_time(&timeline_wrong_name, &entity_path)
        .is_none());
    assert!(store
        .entity_min_time(&timeline_wrong_kind, &entity_path)
        .is_none());
    assert_eq!(
        store.entity_min_time(&timeline_frame_nr, &entity_path),
        Some(TimeInt::new_temporal(32))
    );
    assert_eq!(
        store.entity_min_time(&timeline_log_time, &entity_path),
        Some(TimeInt::try_from(now_minus_one).unwrap())
    );
    assert!(store
        .entity_min_time(&timeline_frame_nr, &wrong_entity_path)
        .is_none());

    Ok(())
}
