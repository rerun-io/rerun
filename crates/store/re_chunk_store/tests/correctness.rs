// https://github.com/rust-lang/rust-clippy/issues/10011
#![cfg(test)]

use std::sync::Arc;

use re_chunk::{Chunk, ChunkId, RowId};
use re_chunk_store::{ChunkStore, ChunkStoreError, LatestAtQuery};
use re_log_types::example_components::{MyIndex, MyPoint};
use re_log_types::{
    build_frame_nr, build_log_time, Duration, EntityPath, Time, TimeInt, TimePoint, TimeType,
    Timeline,
};
use re_types_core::Loggable as _;

// ---

fn query_latest_component<C: re_types_core::Component>(
    store: &ChunkStore,
    entity_path: &EntityPath,
    query: &LatestAtQuery,
) -> Option<(TimeInt, RowId, C)> {
    re_tracing::profile_function!();

    let ((data_time, row_id), unit) = store
        .latest_at_relevant_chunks(query, entity_path, C::name())
        .into_iter()
        .filter_map(|chunk| {
            chunk
                .latest_at(query, C::name())
                .into_unit()
                .and_then(|unit| unit.index(&query.timeline()).map(|index| (index, unit)))
        })
        .max_by_key(|(index, _unit)| *index)?;

    unit.component_mono()?
        .ok()
        .map(|values| (data_time, row_id, values))
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
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(RowId::new(), timepoint.clone(), &[point1])
            .build()?;
        store.insert_chunk(&Arc::new(chunk))?;

        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(RowId::new(), timepoint.clone(), &[point2])
            .build()?;
        store.insert_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            let (_, _, got_point) =
                query_latest_component::<MyPoint>(&store, &entity_path, &query).unwrap();
            similar_asserts::assert_eq!(point2, got_point);
        }
    }

    // * Insert `point1` at frame #10 with a random `RowId`.
    // * Insert `point2` at frame #10 with that same `RowId`.
    // * Nothing happens, as re-using `RowId`s is simply UB.
    {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let row_id = RowId::new();

        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(row_id, timepoint.clone(), &[point1])
            .build()?;
        store.insert_chunk(&Arc::new(chunk))?;

        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(row_id, timepoint.clone(), &[point2])
            .build()?;
        store.insert_chunk(&Arc::new(chunk))?;
    }

    // * Insert `point1` at frame #10 with a random `RowId`.
    // * Insert `point2` at frame #10 using `point1`'s `RowId`, decremented by one.
    // * Query at frame #11 and make sure we get `point1` because of intra-timestamp tie-breaks.
    {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let row_id1 = RowId::new();
        let row_id2 = row_id1.next();

        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(row_id2, timepoint.clone(), &[point1])
            .build()?;
        store.insert_chunk(&Arc::new(chunk))?;

        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(row_id1, timepoint.clone(), &[point2])
            .build()?;
        store.insert_chunk(&Arc::new(chunk))?;

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
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let row_id1 = RowId::new();
        let row_id2 = row_id1.next();

        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(row_id2, TimePoint::default(), &[point1])
            .build()?;
        store.insert_chunk(&Arc::new(chunk))?;

        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(row_id1, TimePoint::default(), &[point2])
            .build()?;
        store.insert_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(Timeline::new_temporal("doesnt_matter"), TimeInt::MAX);
            let (_, _, got_point) =
                query_latest_component::<MyPoint>(&store, &entity_path, &query).unwrap();
            similar_asserts::assert_eq!(point1, got_point);
        }
    }

    // * Insert `point1` at frame #10 with a random `ChunkId` & `RowId`.
    // * Insert `point2` at frame #10 using `point1`'s `ChunkId` & `RowId`.
    // * Query at frame #11 and make sure we get `point1` because chunks are considered idempotent,
    //   and therefore the second write does nothing.
    {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let chunk_id = ChunkId::new();
        let row_id = RowId::new();

        let chunk = Chunk::builder_with_id(chunk_id, entity_path.clone())
            .with_component_batch(row_id, timepoint.clone(), &[point1])
            .build()?;
        store.insert_chunk(&Arc::new(chunk))?;

        let chunk = Chunk::builder_with_id(chunk_id, entity_path.clone())
            .with_component_batch(row_id, timepoint.clone(), &[point2])
            .build()?;
        store.insert_chunk(&Arc::new(chunk))?;

        {
            let query = LatestAtQuery::new(timeline_frame, 11);
            let (_, _, got_point) =
                query_latest_component::<MyPoint>(&store, &entity_path, &query).unwrap();
            similar_asserts::assert_eq!(point1, got_point);
        }
    }

    Ok(())
}

// ---

#[test]
fn write_errors() -> anyhow::Result<()> {
    re_log::setup_logging();

    let entity_path = EntityPath::from("this/that");

    {
        let mut store = ChunkStore::new(
            re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
            Default::default(),
        );

        let row_id1 = RowId::new();
        let row_id2 = row_id1.next();

        let chunk = Chunk::builder(entity_path.clone())
            .with_component_batch(
                row_id2,
                [build_frame_nr(1), build_log_time(Time::now())],
                &MyPoint::from_iter(0..1),
            )
            .with_component_batch(
                row_id1,
                [build_frame_nr(2), build_log_time(Time::now())],
                &MyPoint::from_iter(0..1),
            )
            .build()?;

        assert!(matches!(
            store.insert_chunk(&Arc::new(chunk)),
            Err(ChunkStoreError::UnsortedChunk),
        ));

        Ok(())
    }
}

// ---

#[test]
fn latest_at_emptiness_edge_cases() -> anyhow::Result<()> {
    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );

    let entity_path = EntityPath::from("this/that");
    let now = Time::now();
    let now_minus_1s = now - Duration::from_secs(1.0);
    let now_minus_1s_nanos = now_minus_1s.nanos_since_epoch();
    let frame39 = 39;
    let frame40 = 40;
    let num_instances = 3;

    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(
            RowId::new(),
            [build_log_time(now), build_frame_nr(frame40)],
            &MyIndex::from_iter(0..num_instances),
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    let timeline_wrong_name = Timeline::new("lag_time", TimeType::Time);
    let timeline_wrong_kind = Timeline::new("log_time", TimeType::Sequence);
    let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);
    let timeline_log_time = Timeline::log_time();

    // empty frame_nr
    {
        let chunks = store.latest_at_relevant_chunks(
            &LatestAtQuery::new(timeline_frame_nr, frame39),
            &entity_path,
            MyIndex::name(),
        );
        assert!(chunks.is_empty());
    }

    // empty log_time
    {
        let chunks = store.latest_at_relevant_chunks(
            &LatestAtQuery::new(timeline_log_time, now_minus_1s_nanos),
            &entity_path,
            MyIndex::name(),
        );
        assert!(chunks.is_empty());
    }

    // wrong entity path
    {
        let chunks = store.latest_at_relevant_chunks(
            &LatestAtQuery::new(timeline_frame_nr, frame40),
            &EntityPath::from("does/not/exist"),
            MyIndex::name(),
        );
        assert!(chunks.is_empty());
    }

    // wrong timeline name
    {
        let chunks = store.latest_at_relevant_chunks(
            &LatestAtQuery::new(timeline_wrong_name, frame40),
            &EntityPath::from("does/not/exist"),
            MyIndex::name(),
        );
        assert!(chunks.is_empty());
    }

    // wrong timeline kind
    {
        let chunks = store.latest_at_relevant_chunks(
            &LatestAtQuery::new(timeline_wrong_kind, frame40),
            &EntityPath::from("does/not/exist"),
            MyIndex::name(),
        );
        assert!(chunks.is_empty());
    }

    Ok(())
}

// ---

#[test]
fn entity_min_time_correct() -> anyhow::Result<()> {
    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );

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

    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(
            RowId::new(),
            TimePoint::default()
                .with(timeline_log_time, now)
                .with(timeline_frame_nr, 42),
            &[point],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

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
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(
            RowId::new(),
            TimePoint::default()
                .with(timeline_log_time, now_plus_one)
                .with(timeline_frame_nr, 54),
            &[point],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

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
    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(
            RowId::new(),
            TimePoint::default()
                .with(timeline_log_time, now_minus_one)
                .with(timeline_frame_nr, 32),
            &[point],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

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
