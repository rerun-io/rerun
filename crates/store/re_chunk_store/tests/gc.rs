use std::sync::Arc;

use arrow::array::ArrayRef;
use rand::Rng as _;
use re_chunk::{
    Chunk, ChunkId, ComponentIdentifier, LatestAtQuery, RowId, TimeInt, TimePoint, TimelineName,
};
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, GarbageCollectionOptions, GarbageCollectionTarget, OnMissingChunk,
};
use re_log_types::example_components::{MyColor, MyIndex, MyPoint, MyPoints};
use re_log_types::{AbsoluteTimeRange, EntityPath, Timestamp, build_frame_nr, build_log_time};
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::testing::{build_some_large_structs, large_struct_descriptor};

// ---

fn query_latest_array(
    store: &ChunkStore,
    entity_path: &EntityPath,
    component: ComponentIdentifier,
    query: &LatestAtQuery,
) -> Option<(TimeInt, RowId, ArrayRef)> {
    re_tracing::profile_function!();

    let ((data_time, row_id), unit) = store
        // Purposefully ignoring missing chunks.
        // We know there's going to be missing chunks: it's the whole point of these tests to be
        // removing chunks.
        .latest_at_relevant_chunks(OnMissingChunk::Ignore, query, entity_path, component)
        .chunks
        .into_iter()
        .filter_map(|chunk| {
            let chunk = chunk.latest_at(query, component).into_unit()?;
            chunk.index(&query.timeline()).map(|index| (index, chunk))
        })
        .max_by_key(|(index, _chunk)| *index)?;

    unit.component_batch_raw(component)
        .map(|array| (data_time, row_id, array))
}

// ---

#[test]
fn simple() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut rng = rand::rng();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    for _ in 0..2 {
        let num_ents = 10;
        for i in 0..num_ents {
            let entity_path = EntityPath::from(format!("this/that/{i}"));

            let num_frames = rng.random_range(0..=100);
            let frames = (0..num_frames).filter(|_| rand::rng().random());
            for frame_nr in frames {
                let num_instances = rng.random_range(0..=1_000);
                let chunk = Chunk::builder(entity_path.clone())
                    .with_component_batch(
                        RowId::new(),
                        [build_frame_nr(frame_nr)],
                        (
                            large_struct_descriptor(),
                            &build_some_large_structs(num_instances),
                        ),
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
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
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
                [
                    (MyIndex::partial_descriptor(), &indices1 as _),
                    (MyPoints::descriptor_colors(), &colors1 as _),
                ],
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
                [
                    (MyIndex::partial_descriptor(), &indices1 as _),
                    (MyPoints::descriptor_points(), &points2 as _),
                ],
            )
            .build()?,
    );

    let points3 = MyPoint::from_iter(0..10);
    let chunk3 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            RowId::new(),
            [build_frame_nr(frame3)],
            [(MyPoints::descriptor_points(), &points3 as _)],
        )
        .build()?;

    let colors4 = MyColor::from_iter(0..5);
    let chunk4 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            RowId::new(),
            [build_frame_nr(frame4)],
            [(MyPoints::descriptor_colors(), &colors4 as _)],
        )
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

    let assert_latest_components = |frame_nr: TimeInt, rows: &[(ComponentDescriptor, RowId)]| {
        let timeline_frame_nr = TimelineName::new("frame_nr");

        for (component_descr, expected_row_id) in rows {
            let (_data_time, row_id, _array) = query_latest_array(
                &store,
                &entity_path,
                component_descr.component,
                &LatestAtQuery::new(timeline_frame_nr, frame_nr),
            )
            .unwrap();

            assert_eq!(*expected_row_id, row_id, "{component_descr}");
        }
    };

    eprintln!("{store}");

    assert_latest_components(
        TimeInt::MAX,
        &[
            (MyIndex::partial_descriptor(), row_id2_static),
            (MyPoints::descriptor_colors(), row_id1_static),
            (MyPoints::descriptor_points(), row_id2_static),
        ],
    );

    Ok(())
}

#[test]
fn protected() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
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
            [
                (MyIndex::partial_descriptor(), &indices1 as _),
                (MyPoints::descriptor_colors(), &colors1 as _),
            ],
        )
        .build()?;

    let row_id2 = RowId::new();
    let points2 = MyPoint::from_iter(0..3);
    let chunk2 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id2,
            [build_frame_nr(frame2)],
            [
                (MyIndex::partial_descriptor(), &indices1 as _),
                (MyPoints::descriptor_points(), &points2 as _),
            ],
        )
        .build()?;

    let row_id3 = RowId::new();
    let points3 = MyPoint::from_iter(0..10);
    let chunk3 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id3,
            [build_frame_nr(frame3)],
            [(MyPoints::descriptor_points(), &points3 as _)],
        )
        .build()?;

    let row_id4 = RowId::new();
    let colors4 = MyColor::from_iter(0..5);
    let chunk4 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id4,
            [build_frame_nr(frame4)],
            [(MyPoints::descriptor_colors(), &colors4 as _)],
        )
        .build()?;

    store.insert_chunk(&Arc::new(chunk1))?;
    store.insert_chunk(&Arc::new(chunk2))?;
    store.insert_chunk(&Arc::new(chunk3))?;
    store.insert_chunk(&Arc::new(chunk4))?;

    store.gc(&GarbageCollectionOptions {
        protect_latest: 1,
        ..GarbageCollectionOptions::gc_everything()
    });

    let assert_latest_components =
        |frame_nr: TimeInt, rows: &[(ComponentDescriptor, Option<RowId>)]| {
            let timeline_frame_nr = TimelineName::new("frame_nr");

            for (component_descr, expected_row_id) in rows {
                let row_id = query_latest_array(
                    &store,
                    &entity_path,
                    component_descr.component,
                    &LatestAtQuery::new(timeline_frame_nr, frame_nr),
                )
                .map(|(_data_time, row_id, _array)| row_id);

                assert_eq!(*expected_row_id, row_id, "{component_descr}");
            }
        };

    eprintln!("{store}");

    assert_latest_components(
        frame1,
        &[
            (MyIndex::partial_descriptor(), None),
            (MyPoints::descriptor_colors(), None),
            (MyPoints::descriptor_points(), None),
        ],
    );

    assert_latest_components(
        frame2,
        &[
            (MyIndex::partial_descriptor(), Some(row_id2)),
            (MyPoints::descriptor_colors(), None),
            (MyPoints::descriptor_points(), Some(row_id2)),
        ],
    );

    assert_latest_components(
        frame3,
        &[
            (MyIndex::partial_descriptor(), Some(row_id2)),
            (MyPoints::descriptor_colors(), None),
            (MyPoints::descriptor_points(), Some(row_id3)),
        ],
    );

    assert_latest_components(
        frame4,
        &[
            (MyIndex::partial_descriptor(), Some(row_id2)),
            (MyPoints::descriptor_colors(), Some(row_id4)),
            (MyPoints::descriptor_points(), Some(row_id3)),
        ],
    );

    Ok(())
}

#[test]
fn protected_time_ranges() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
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
            [
                (MyIndex::partial_descriptor(), &indices1 as _),
                (MyPoints::descriptor_colors(), &colors1 as _),
            ],
        )
        .build()?;

    let row_id2 = RowId::new();
    let points2 = MyPoint::from_iter(0..3);
    let chunk2 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id2,
            [build_frame_nr(frame2)],
            [
                (MyIndex::partial_descriptor(), &indices1 as _),
                (MyPoints::descriptor_points(), &points2 as _),
            ],
        )
        .build()?;

    let row_id3 = RowId::new();
    let points3 = MyPoint::from_iter(0..10);
    let chunk3 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id3,
            [build_frame_nr(frame3)],
            [(MyPoints::descriptor_points(), &points3 as _)],
        )
        .build()?;

    let row_id4 = RowId::new();
    let colors4 = MyColor::from_iter(0..5);
    let chunk4 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id4,
            [build_frame_nr(frame4)],
            [(MyPoints::descriptor_colors(), &colors4 as _)],
        )
        .build()?;

    let chunk1 = Arc::new(chunk1);
    let chunk2 = Arc::new(chunk2);
    let chunk3 = Arc::new(chunk3);
    let chunk4 = Arc::new(chunk4);

    store.insert_chunk(&chunk1)?;
    store.insert_chunk(&chunk2)?;
    store.insert_chunk(&chunk3)?;
    store.insert_chunk(&chunk4)?;

    fn protect_time_range(time_range: AbsoluteTimeRange) -> GarbageCollectionOptions {
        GarbageCollectionOptions {
            protected_time_ranges: std::iter::once((TimelineName::new("frame_nr"), time_range))
                .collect(),
            ..GarbageCollectionOptions::gc_everything()
        }
    }

    eprintln!("{store}");

    let (events, _) = store.gc(&protect_time_range(AbsoluteTimeRange::new(1, 4)));
    assert_eq!(events.len(), 0);

    let (events, _) = store.gc(&protect_time_range(AbsoluteTimeRange::new(2, 4)));
    assert_eq!(events.len(), 1);
    assert!(Arc::ptr_eq(events[0].diff.delta_chunk().unwrap(), &chunk1));

    let (events, _) = store.gc(&protect_time_range(AbsoluteTimeRange::new(2, 3)));
    assert_eq!(events.len(), 1);
    assert!(Arc::ptr_eq(events[0].diff.delta_chunk().unwrap(), &chunk4));

    Ok(())
}

// ---

#[test]
fn manual_drop_entity_path() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        Default::default(),
    );

    let entity_path1 = EntityPath::from("entity1");
    let entity_path2 = EntityPath::from("entity1/entity2");

    let row_id1 = RowId::new();
    let indices1 = MyIndex::from_iter(0..3);
    let chunk1 = Arc::new(
        Chunk::builder(entity_path1.clone())
            .with_component_batches(
                row_id1,
                [build_frame_nr(10)],
                [(MyIndex::partial_descriptor(), &indices1 as _)],
            )
            .build()?,
    );

    let row_id2 = RowId::new();
    let indices2 = MyIndex::from_iter(0..3);
    let chunk2 = Arc::new(
        Chunk::builder(entity_path1.clone())
            .with_component_batches(
                row_id2,
                [build_log_time(Timestamp::now())],
                [(MyIndex::partial_descriptor(), &indices2 as _)],
            )
            .build()?,
    );

    let row_id3 = RowId::new();
    let indices3 = MyIndex::from_iter(0..3);
    let chunk3 = Arc::new(
        Chunk::builder(entity_path1.clone())
            .with_component_batches(
                row_id3,
                TimePoint::default(),
                [(MyIndex::partial_descriptor(), &indices3 as _)],
            )
            .build()?,
    );

    let row_id4 = RowId::new();
    let indices4 = MyIndex::from_iter(0..3);
    let chunk4 = Arc::new(
        Chunk::builder(entity_path2.clone())
            .with_component_batches(
                row_id4,
                [build_frame_nr(42), build_log_time(Timestamp::now())],
                [(MyIndex::partial_descriptor(), &indices4 as _)],
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
        let row_id = query_latest_array(
            store,
            entity_path,
            MyIndex::partial_descriptor().component,
            query,
        )
        .map(|(_data_time, row_id, _array)| row_id);

        assert_eq!(expected_row_id, row_id);
    };

    assert_latest_value(
        &store,
        &entity_path1,
        &LatestAtQuery::new(TimelineName::new("frame_nr"), TimeInt::MAX),
        Some(row_id3),
    );
    assert_latest_value(
        &store,
        &entity_path1,
        &LatestAtQuery::new(TimelineName::log_time(), TimeInt::MAX),
        Some(row_id3),
    );

    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(TimelineName::new("frame_nr"), TimeInt::MAX),
        Some(row_id4),
    );
    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(TimelineName::log_time(), TimeInt::MAX),
        Some(row_id4),
    );

    let events = store.drop_entity_path(&entity_path1);
    assert_eq!(3, events.len());
    assert!(events[0].is_deletion());
    assert!(events[1].is_deletion());
    assert!(events[2].is_deletion());
    similar_asserts::assert_eq!(
        &chunk3, /* static comes first */
        events[0].delta_chunk().unwrap()
    );
    similar_asserts::assert_eq!(&chunk1, events[1].delta_chunk().unwrap());
    similar_asserts::assert_eq!(&chunk2, events[2].delta_chunk().unwrap());

    assert_latest_value(
        &store,
        &entity_path1,
        &LatestAtQuery::new(TimelineName::new("frame_nr"), TimeInt::MAX),
        None,
    );
    assert_latest_value(
        &store,
        &entity_path1,
        &LatestAtQuery::new(TimelineName::log_time(), TimeInt::MAX),
        None,
    );

    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(TimelineName::new("frame_nr"), TimeInt::MAX),
        Some(row_id4),
    );
    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(TimelineName::log_time(), TimeInt::MAX),
        Some(row_id4),
    );

    let events = store.drop_entity_path(&entity_path1);
    assert!(events.is_empty());

    let events = store.drop_entity_path(&entity_path2);
    assert_eq!(1, events.len());
    assert!(events[0].is_deletion());
    similar_asserts::assert_eq!(&chunk4, events[0].delta_chunk().unwrap());

    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(TimelineName::new("frame_nr"), TimeInt::MAX),
        None,
    );
    assert_latest_value(
        &store,
        &entity_path2,
        &LatestAtQuery::new(TimelineName::log_time(), TimeInt::MAX),
        None,
    );

    let events = store.drop_entity_path(&entity_path2);
    assert!(events.is_empty());

    Ok(())
}
