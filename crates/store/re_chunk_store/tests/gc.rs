use std::sync::Arc;

use arrow2::array::Array as Arrow2Array;
use rand::Rng as _;

use re_chunk::{Chunk, ChunkId, ComponentName, LatestAtQuery, RowId, TimeInt, TimePoint};
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, ChunkStoreDiffKind, GarbageCollectionOptions,
    GarbageCollectionTarget,
};
use re_log_types::{
    build_frame_nr, build_log_time,
    example_components::{MyColor, MyIndex, MyPoint},
    EntityPath, ResolvedTimeRange, Time, TimeType, Timeline,
};
use re_types::testing::build_some_large_structs;
use re_types_core::Component as _;

// ---

fn query_latest_array(
    store: &ChunkStore,
    entity_path: &EntityPath,
    component_name: ComponentName,
    query: &LatestAtQuery,
) -> Option<(TimeInt, RowId, Box<dyn Arrow2Array>)> {
    re_tracing::profile_function!();

    let ((data_time, row_id), unit) = store
        .latest_at_relevant_chunks(query, entity_path, component_name)
        .into_iter()
        .filter_map(|chunk| {
            chunk
                .latest_at(query, component_name)
                .into_unit()
                .and_then(|chunk| chunk.index(&query.timeline()).map(|index| (index, chunk)))
        })
        .max_by_key(|(index, _chunk)| *index)?;

    unit.component_batch_raw_arrow2(&component_name)
        .map(|array| (data_time, row_id, array))
}

// ---

#[test]
fn simple() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut rng = rand::thread_rng();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    for _ in 0..2 {
        let num_ents = 10;
        for i in 0..num_ents {
            let entity_path = EntityPath::from(format!("this/that/{i}"));

            let num_frames = rng.gen_range(0..=100);
            let frames = (0..num_frames).filter(|_| rand::thread_rng().gen());
            for frame_nr in frames {
                let num_instances = rng.gen_range(0..=1_000);
                let chunk = Chunk::builder(entity_path.clone())
                    .with_component_batch(
                        RowId::new(),
                        [build_frame_nr(frame_nr)],
                        &build_some_large_structs(num_instances),
                    )
                    .build()?;
                store.insert_chunk(&Arc::new(chunk))?;
            }
        }

        let stats_before = store.stats();

        let (_store_events, stats_diff) = store.gc(&GarbageCollectionOptions {
            target: GarbageCollectionTarget::DropAtLeastFraction(1.0 / 3.0),
            ..GarbageCollectionOptions::gc_everything()
        });

        // NOTE: only temporal data gets purged!
        let num_bytes_dropped = stats_diff.total().total_size_bytes as f64;
        let num_bytes_dropped_expected_min =
            stats_before.total().total_size_bytes as f64 * 0.95 / 3.0;
        let num_bytes_dropped_expected_max =
            stats_before.total().total_size_bytes as f64 * 1.05 / 3.0;

        assert!(
            num_bytes_dropped_expected_min <= num_bytes_dropped
                && num_bytes_dropped <= num_bytes_dropped_expected_max,
            "{} <= {} <= {}",
            re_format::format_bytes(num_bytes_dropped_expected_min),
            re_format::format_bytes(num_bytes_dropped),
            re_format::format_bytes(num_bytes_dropped_expected_max),
        );
    }

    Ok(())
}

#[test]
fn simple_static() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );

    let entity_path = EntityPath::from("this/that");

    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);

    let row_id1 = RowId::new();
    let (indices1, colors1) = (MyIndex::from_iter(0..3), MyColor::from_iter(0..3));
    let chunk1 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id1,
                [build_frame_nr(frame1)],
                [&indices1 as _, &colors1 as _],
            )
            .build()?,
    );

    let row_id2 = RowId::new();
    let points2 = MyPoint::from_iter(0..3);
    let chunk2 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id2,
                [build_frame_nr(frame2)],
                [&indices1 as _, &points2 as _],
            )
            .build()?,
    );

    let points3 = MyPoint::from_iter(0..10);
    let chunk3 = Chunk::builder(entity_path.clone())
        .with_component_batches(RowId::new(), [build_frame_nr(frame3)], [&points3 as _])
        .build()?;

    let colors4 = MyColor::from_iter(0..5);
    let chunk4 = Chunk::builder(entity_path.clone())
        .with_component_batches(RowId::new(), [build_frame_nr(frame4)], [&colors4 as _])
        .build()?;

    store.insert_chunk(&chunk1)?;
    store.insert_chunk(&chunk2)?;
    store.insert_chunk(&Arc::new(chunk3))?;
    store.insert_chunk(&Arc::new(chunk4))?;

    // Re-insert `chunk1` and `chunk2` as static data as well
    let row_id1_static = RowId::new();
    let chunk1_static = chunk1
        .clone_as(ChunkId::new(), row_id1_static)
        .into_static();
    let row_id2_static = RowId::new();
    let chunk2_static = chunk2
        .clone_as(ChunkId::new(), row_id2_static)
        .into_static();
    store.insert_chunk(&Arc::new(chunk1_static))?;
    store.insert_chunk(&Arc::new(chunk2_static))?;

    store.gc(&GarbageCollectionOptions {
        protect_latest: 1,
        ..GarbageCollectionOptions::gc_everything()
    });

    let assert_latest_components = |frame_nr: TimeInt, rows: &[(ComponentName, RowId)]| {
        let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

        for (component_name, expected_row_id) in rows {
            let (_data_time, row_id, _array) = query_latest_array(
                &store,
                &entity_path,
                *component_name,
                &LatestAtQuery::new(timeline_frame_nr, frame_nr),
            )
            .unwrap();

            assert_eq!(*expected_row_id, row_id, "{component_name}");
        }
    };

    eprintln!("{store}");

    assert_latest_components(
        TimeInt::MAX,
        &[
            (MyIndex::name(), row_id2_static),
            (MyColor::name(), row_id1_static),
            (MyPoint::name(), row_id2_static),
        ],
    );

    Ok(())
}

#[test]
fn protected() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let entity_path = EntityPath::from("this/that");

    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);

    let row_id1 = RowId::new();
    let (indices1, colors1) = (MyIndex::from_iter(0..3), MyColor::from_iter(0..3));
    let chunk1 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id1,
            [build_frame_nr(frame1)],
            [&indices1 as _, &colors1 as _],
        )
        .build()?;

    let row_id2 = RowId::new();
    let points2 = MyPoint::from_iter(0..3);
    let chunk2 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id2,
            [build_frame_nr(frame2)],
            [&indices1 as _, &points2 as _],
        )
        .build()?;

    let row_id3 = RowId::new();
    let points3 = MyPoint::from_iter(0..10);
    let chunk3 = Chunk::builder(entity_path.clone())
        .with_component_batches(row_id3, [build_frame_nr(frame3)], [&points3 as _])
        .build()?;

    let row_id4 = RowId::new();
    let colors4 = MyColor::from_iter(0..5);
    let chunk4 = Chunk::builder(entity_path.clone())
        .with_component_batches(row_id4, [build_frame_nr(frame4)], [&colors4 as _])
        .build()?;

    store.insert_chunk(&Arc::new(chunk1))?;
    store.insert_chunk(&Arc::new(chunk2))?;
    store.insert_chunk(&Arc::new(chunk3))?;
    store.insert_chunk(&Arc::new(chunk4))?;

    store.gc(&GarbageCollectionOptions {
        protect_latest: 1,
        ..GarbageCollectionOptions::gc_everything()
    });

    let assert_latest_components = |frame_nr: TimeInt, rows: &[(ComponentName, Option<RowId>)]| {
        let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

        for (component_name, expected_row_id) in rows {
            let row_id = query_latest_array(
                &store,
                &entity_path,
                *component_name,
                &LatestAtQuery::new(timeline_frame_nr, frame_nr),
            )
            .map(|(_data_time, row_id, _array)| row_id);

            assert_eq!(*expected_row_id, row_id, "{component_name}");
        }
    };

    eprintln!("{store}");

    assert_latest_components(
        frame1,
        &[
            (MyIndex::name(), None),
            (MyColor::name(), None),
            (MyPoint::name(), None),
        ],
    );

    assert_latest_components(
        frame2,
        &[
            (MyIndex::name(), Some(row_id2)),
            (MyColor::name(), None),
            (MyPoint::name(), Some(row_id2)),
        ],
    );

    assert_latest_components(
        frame3,
        &[
            (MyIndex::name(), Some(row_id2)),
            (MyColor::name(), None),
            (MyPoint::name(), Some(row_id3)),
        ],
    );

    assert_latest_components(
        frame4,
        &[
            (MyIndex::name(), Some(row_id2)),
            (MyColor::name(), Some(row_id4)),
            (MyPoint::name(), Some(row_id3)),
        ],
    );

    Ok(())
}

#[test]
fn protected_time_ranges() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let entity_path = EntityPath::from("this/that");

    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);

    let row_id1 = RowId::new();
    let (indices1, colors1) = (MyIndex::from_iter(0..3), MyColor::from_iter(0..3));
    let chunk1 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id1,
            [build_frame_nr(frame1)],
            [&indices1 as _, &colors1 as _],
        )
        .build()?;

    let row_id2 = RowId::new();
    let points2 = MyPoint::from_iter(0..3);
    let chunk2 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id2,
            [build_frame_nr(frame2)],
            [&indices1 as _, &points2 as _],
        )
        .build()?;

    let row_id3 = RowId::new();
    let points3 = MyPoint::from_iter(0..10);
    let chunk3 = Chunk::builder(entity_path.clone())
        .with_component_batches(row_id3, [build_frame_nr(frame3)], [&points3 as _])
        .build()?;

    let row_id4 = RowId::new();
    let colors4 = MyColor::from_iter(0..5);
    let chunk4 = Chunk::builder(entity_path.clone())
        .with_component_batches(row_id4, [build_frame_nr(frame4)], [&colors4 as _])
        .build()?;

    let chunk1 = Arc::new(chunk1);
    let chunk2 = Arc::new(chunk2);
    let chunk3 = Arc::new(chunk3);
    let chunk4 = Arc::new(chunk4);

    store.insert_chunk(&chunk1)?;
    store.insert_chunk(&chunk2)?;
    store.insert_chunk(&chunk3)?;
    store.insert_chunk(&chunk4)?;

    fn protect_time_range(time_range: ResolvedTimeRange) -> GarbageCollectionOptions {
        GarbageCollectionOptions {
            protected_time_ranges: std::iter::once((
                Timeline::new_sequence("frame_nr"),
                time_range,
            ))
            .collect(),
            ..GarbageCollectionOptions::gc_everything()
        }
    }

    eprintln!("{store}");

    let (events, _) = store.gc(&protect_time_range(ResolvedTimeRange::new(1, 4)));
    assert_eq!(events.len(), 0);

    let (events, _) = store.gc(&protect_time_range(ResolvedTimeRange::new(2, 4)));
    assert_eq!(events.len(), 1);
    assert!(Arc::ptr_eq(&events[0].diff.chunk, &chunk1));

    let (events, _) = store.gc(&protect_time_range(ResolvedTimeRange::new(2, 3)));
    assert_eq!(events.len(), 1);
    assert!(Arc::ptr_eq(&events[0].diff.chunk, &chunk4));

    Ok(())
}

// ---

#[test]
fn manual_drop_entity_path() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        Default::default(),
    );

    let entity_path1 = EntityPath::from("entity1");
    let entity_path2 = EntityPath::from("entity1/entity2");

    let row_id1 = RowId::new();
    let indices1 = MyIndex::from_iter(0..3);
    let chunk1 = Arc::new(
        Chunk::builder(entity_path1.clone())
            .with_component_batches(row_id1, [build_frame_nr(10)], [&indices1 as _])
            .build()?,
    );

    let row_id2 = RowId::new();
    let indices2 = MyIndex::from_iter(0..3);
    let chunk2 = Arc::new(
        Chunk::builder(entity_path1.clone())
            .with_component_batches(row_id2, [build_log_time(Time::now())], [&indices2 as _])
            .build()?,
    );

    let row_id3 = RowId::new();
    let indices3 = MyIndex::from_iter(0..3);
    let chunk3 = Arc::new(
        Chunk::builder(entity_path1.clone())
            .with_component_batches(row_id3, TimePoint::default(), [&indices3 as _])
            .build()?,
    );

    let row_id4 = RowId::new();
    let indices4 = MyIndex::from_iter(0..3);
    let chunk4 = Arc::new(
        Chunk::builder(entity_path2.clone())
            .with_component_batches(
                row_id4,
                [build_frame_nr(42), build_log_time(Time::now())],
                [&indices4 as _],
            )
            .build()?,
    );

    store.insert_chunk(&chunk1)?;
    store.insert_chunk(&chunk2)?;
    store.insert_chunk(&chunk3)?;
    store.insert_chunk(&chunk4)?;

    let assert_latest_value = |store: &ChunkStore,
                               entity_path: &EntityPath,
                               query: &LatestAtQuery,
                               expected_row_id: Option<RowId>| {
        let row_id = query_latest_array(store, entity_path, MyIndex::name(), query)
            .map(|(_data_time, row_id, _array)| row_id);

        assert_eq!(expected_row_id, row_id);
    };

    assert_latest_value(
        &store,
        &entity_path1,
        &LatestAtQuery::new(Timeline::new_sequence("frame_nr"), TimeInt::MAX),
        Some(row_id3),
    );
    assert_latest_value(
        &store,
        &entity_path1,
        &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
        Some(row_id3),
    );

    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(Timeline::new_sequence("frame_nr"), TimeInt::MAX),
        Some(row_id4),
    );
    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
        Some(row_id4),
    );

    let events = store.drop_entity_path(&entity_path1);
    assert_eq!(3, events.len());
    assert_eq!(ChunkStoreDiffKind::Deletion, events[0].kind);
    assert_eq!(ChunkStoreDiffKind::Deletion, events[1].kind);
    assert_eq!(ChunkStoreDiffKind::Deletion, events[2].kind);
    similar_asserts::assert_eq!(chunk3 /* static comes first */, events[0].chunk);
    similar_asserts::assert_eq!(chunk1, events[1].chunk);
    similar_asserts::assert_eq!(chunk2, events[2].chunk);

    assert_latest_value(
        &store,
        &entity_path1,
        &LatestAtQuery::new(Timeline::new_sequence("frame_nr"), TimeInt::MAX),
        None,
    );
    assert_latest_value(
        &store,
        &entity_path1,
        &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
        None,
    );

    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(Timeline::new_sequence("frame_nr"), TimeInt::MAX),
        Some(row_id4),
    );
    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
        Some(row_id4),
    );

    let events = store.drop_entity_path(&entity_path1);
    assert!(events.is_empty());

    let events = store.drop_entity_path(&entity_path2);
    assert_eq!(1, events.len());
    assert_eq!(ChunkStoreDiffKind::Deletion, events[0].kind);
    similar_asserts::assert_eq!(chunk4, events[0].chunk);

    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(Timeline::new_sequence("frame_nr"), TimeInt::MAX),
        None,
    );
    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(Timeline::log_time(), TimeInt::MAX),
        None,
    );

    let events = store.drop_entity_path(&entity_path2);
    assert!(events.is_empty());

    Ok(())
}
