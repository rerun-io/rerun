use std::sync::Arc;

use arrow::array::ArrayRef;
use itertools::Itertools as _;
use re_chunk::{Chunk, ChunkId, RowId, TimePoint, TimelineName};
use re_chunk_store::{
    AbsoluteTimeRange, ChunkStore, ChunkStoreConfig, LatestAtQuery, RangeQuery, TimeInt,
};
use re_log_types::example_components::{MyColor, MyIndex, MyPoint, MyPoints};
use re_log_types::{EntityPath, TimeType, Timeline, build_frame_nr};
use re_sdk_types::testing::{build_some_large_structs, large_struct_descriptor};
use re_sdk_types::{ComponentDescriptor, ComponentSet};

// ---

#[expect(clippy::unwrap_used)]
fn query_latest_array(
    store: &ChunkStore,
    entity_path: &EntityPath,
    component_descr: &ComponentDescriptor,
    query: &LatestAtQuery,
) -> Option<(TimeInt, RowId, ArrayRef)> {
    re_tracing::profile_function!();

    let ((data_time, row_id), unit) = store
        .latest_at_relevant_chunks(query, entity_path, component_descr.component)
        .to_iter()
        .unwrap()
        .filter_map(|chunk| {
            let chunk = chunk
                .latest_at(query, component_descr.component)
                .into_unit()?;
            chunk.index(&query.timeline()).map(|index| (index, chunk))
        })
        .max_by_key(|(index, _chunk)| *index)?;

    unit.component_batch_raw(component_descr.component)
        .map(|array| (data_time, row_id, array))
}

// ---

#[test]
fn all_components() -> anyhow::Result<()> {
    re_log::setup_logging();

    let entity_path = EntityPath::from("this/that");

    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);

    let assert_latest_components_at =
        |store: &ChunkStore, entity_path: &EntityPath, expected: Option<&[ComponentDescriptor]>| {
            let timeline = TimelineName::new("frame_nr");

            let components = store.all_components_on_timeline_sorted(&timeline, entity_path);

            let expected_components = expected.map(|expected| {
                let expected: ComponentSet = expected.iter().map(|descr| descr.component).collect();
                expected
            });

            assert_eq!(
                expected_components, components,
                "expected to find {expected_components:?}, found {components:?} instead\n{store}",
            );
        };

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let components_a = &[
        MyPoints::descriptor_colors(), // added by test, static
        large_struct_descriptor(),     // added by test
    ];

    let components_b = &[
        MyPoints::descriptor_colors(), // added by test, static
        MyPoints::descriptor_points(), // added by test
        large_struct_descriptor(),     // added by test
    ];

    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(
            RowId::new(),
            TimePoint::default(),
            (MyPoints::descriptor_colors(), &MyColor::from_iter(0..2)),
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(
            RowId::new(),
            [build_frame_nr(frame1)],
            (large_struct_descriptor(), &build_some_large_structs(2)),
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    assert_latest_components_at(&mut store, &entity_path, Some(components_a));

    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batches(
            RowId::new(),
            [build_frame_nr(frame2)],
            [
                (large_struct_descriptor(), &build_some_large_structs(2) as _),
                (
                    MyPoints::descriptor_points(),
                    &MyPoint::from_iter(0..2) as _,
                ),
            ],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    assert_latest_components_at(&mut store, &entity_path, Some(components_b));

    Ok(())
}

#[test]
fn test_all_components_on_timeline() -> anyhow::Result<()> {
    re_log::setup_logging();

    let entity_path1 = EntityPath::from("both/timeline");
    let entity_path2 = EntityPath::from("only/timeline1");

    let timeline1 = Timeline::new("timeline1", TimeType::Sequence);
    let timeline2 = Timeline::new("timeline2", TimeType::Sequence);

    let time = TimeInt::new_temporal(1);

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let chunk = Chunk::builder(entity_path1.clone())
        .with_component_batch(
            RowId::new(),
            [(timeline1, time), (timeline2, time)],
            (large_struct_descriptor(), &build_some_large_structs(2)),
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    let chunk = Chunk::builder(entity_path2.clone())
        .with_component_batches(
            RowId::new(),
            [(timeline1, time)],
            [(large_struct_descriptor(), &build_some_large_structs(2) as _)],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    // entity1 is on both timelines
    assert!(
        !store
            .all_components_on_timeline(timeline1.name(), &entity_path1)
            .unwrap()
            .is_empty()
    );
    assert!(
        !store
            .all_components_on_timeline(timeline2.name(), &entity_path1)
            .unwrap()
            .is_empty()
    );

    // entity2 is only on timeline1
    assert!(
        !store
            .all_components_on_timeline(timeline1.name(), &entity_path2)
            .unwrap()
            .is_empty()
    );

    assert!(
        store
            .all_components_on_timeline(timeline2.name(), &entity_path2)
            .is_none()
    );

    Ok(())
}

// ---

#[test]
fn latest_at() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let entity_path = EntityPath::from("this/that");

    let frame0 = TimeInt::new_temporal(0);
    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);
    let frame5 = TimeInt::new_temporal(5);

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

    // injecting some static colors
    let row_id5 = RowId::new();
    let colors5 = MyColor::from_iter(0..3);
    let chunk5 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id5,
            TimePoint::default(),
            [(MyPoints::descriptor_colors(), &colors5 as _)],
        )
        .build()?;

    let chunk1 = Arc::new(chunk1);
    let chunk2 = Arc::new(chunk2);
    let chunk3 = Arc::new(chunk3);
    let chunk4 = Arc::new(chunk4);
    let chunk5 = Arc::new(chunk5);

    store.insert_chunk(&chunk1)?;
    store.insert_chunk(&chunk2)?;
    store.insert_chunk(&chunk3)?;
    store.insert_chunk(&chunk4)?;
    store.insert_chunk(&chunk5)?;

    let assert_latest_components =
        |frame_nr: TimeInt, rows: &[(ComponentDescriptor, Option<RowId>)]| {
            let timeline_frame_nr = TimelineName::new("frame_nr");

            for (component_desc, expected_row_id) in rows {
                let row_id = query_latest_array(
                    &store,
                    &entity_path,
                    component_desc,
                    &LatestAtQuery::new(timeline_frame_nr, frame_nr),
                )
                .map(|(_data_time, row_id, _array)| row_id);

                assert_eq!(*expected_row_id, row_id, "{component_desc}");
            }
        };

    assert_latest_components(
        frame0,
        &[
            (MyPoints::descriptor_colors(), Some(row_id5)), // static
            (MyIndex::partial_descriptor(), None),
            (MyPoints::descriptor_points(), None),
        ],
    );
    assert_latest_components(
        frame1,
        &[
            (MyPoints::descriptor_colors(), Some(row_id5)), // static
            (MyIndex::partial_descriptor(), Some(row_id1)),
            (MyPoints::descriptor_points(), None),
        ],
    );
    assert_latest_components(
        frame2,
        &[
            (MyPoints::descriptor_colors(), Some(row_id5)),
            (MyPoints::descriptor_points(), Some(row_id2)),
            (MyIndex::partial_descriptor(), Some(row_id2)),
        ],
    );
    assert_latest_components(
        frame3,
        &[
            (MyPoints::descriptor_colors(), Some(row_id5)),
            (MyPoints::descriptor_points(), Some(row_id3)),
            (MyIndex::partial_descriptor(), Some(row_id2)),
        ],
    );
    assert_latest_components(
        frame4,
        &[
            (MyPoints::descriptor_colors(), Some(row_id5)),
            (MyPoints::descriptor_points(), Some(row_id3)),
            (MyIndex::partial_descriptor(), Some(row_id2)),
        ],
    );

    // Component-less APIs
    {
        let assert_latest_chunk =
            |store: &ChunkStore, frame_nr: TimeInt, mut expected_chunk_ids: Vec<ChunkId>| {
                let timeline_frame_nr = TimelineName::new("frame_nr");

                let mut chunk_ids = store
                    .latest_at_relevant_chunks_for_all_components(
                        &LatestAtQuery::new(timeline_frame_nr, frame_nr),
                        &entity_path,
                        false, /* don't include static data */
                    )
                    .to_iter()
                    .unwrap()
                    .map(|chunk| chunk.id())
                    .collect_vec();
                chunk_ids.sort();

                expected_chunk_ids.sort();

                similar_asserts::assert_eq!(expected_chunk_ids, chunk_ids);
            };

        assert_latest_chunk(&store, frame0, vec![]);
        assert_latest_chunk(&store, frame1, vec![chunk1.id()]);
        assert_latest_chunk(&store, frame2, vec![chunk2.id()]);
        assert_latest_chunk(&store, frame3, vec![chunk3.id()]);
        assert_latest_chunk(&store, frame4, vec![chunk4.id()]);
        assert_latest_chunk(&store, frame5, vec![chunk4.id()]);
    }

    Ok(())
}

#[test]
fn latest_at_sparse_component_edge_case() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let entity_path = EntityPath::from("this/that");

    let frame0 = TimeInt::new_temporal(0);
    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);

    // This chunk has a time range of `(1, 3)`, but the actual data for `MyIndex` actually only
    // starts at `3`.

    let row_id1_1 = RowId::new();
    let row_id1_2 = RowId::new();
    let row_id1_3 = RowId::new();
    let chunk1 = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id1_1,
            [build_frame_nr(frame1)],
            [
                (MyIndex::partial_descriptor(), None),
                (
                    MyPoints::descriptor_points(),
                    Some(&MyPoint::from_iter(0..1) as _),
                ),
            ],
        )
        .with_sparse_component_batches(
            row_id1_2,
            [build_frame_nr(frame2)],
            [
                (MyIndex::partial_descriptor(), None),
                (
                    MyPoints::descriptor_points(),
                    Some(&MyPoint::from_iter(1..2) as _),
                ),
            ],
        )
        .with_sparse_component_batches(
            row_id1_3,
            [build_frame_nr(frame3)],
            [
                (
                    MyIndex::partial_descriptor(),
                    Some(&MyIndex::from_iter(2..3) as _),
                ),
                (
                    MyPoints::descriptor_points(),
                    Some(&MyPoint::from_iter(2..3) as _),
                ),
            ],
        )
        .build()?;

    let chunk1 = Arc::new(chunk1);
    eprintln!("chunk 1:\n{chunk1}");
    store.insert_chunk(&chunk1)?;

    // This chunk on the other hand has a time range of `(2, 3)`, and the data for `MyIndex`
    // actually does start at `2`.

    let row_id2_1 = RowId::new();
    let chunk2 = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id2_1,
            [build_frame_nr(frame2)],
            [
                (
                    MyIndex::partial_descriptor(),
                    Some(&MyIndex::from_iter(2..3) as _),
                ),
                (
                    MyPoints::descriptor_points(),
                    Some(&MyPoint::from_iter(1..2) as _),
                ),
            ],
        )
        .build()?;

    let chunk2 = Arc::new(chunk2);
    eprintln!("chunk 2:\n{chunk2}");
    store.insert_chunk(&chunk2)?;

    // We expect the data for `MyIndex` to come from `row_id_1_3`, since it is the most recent
    // piece of data.
    // The only way this can happen is if we have proper per-component time-ranges, since a global
    // per-chunk time-range would erroneously push us towards the second chunk.

    let row_id = query_latest_array(
        &store,
        &entity_path,
        &MyIndex::partial_descriptor(),
        &LatestAtQuery::new(TimelineName::new("frame_nr"), TimeInt::MAX),
    )
    .map(|(_data_time, row_id, _array)| row_id);

    assert_eq!(row_id1_3, row_id.unwrap());

    // Component-less APIs
    {
        let assert_latest_chunk = |frame_nr: TimeInt, mut expected_chunk_ids: Vec<ChunkId>| {
            let timeline_frame_nr = TimelineName::new("frame_nr");

            eprintln!("--- {frame_nr:?} ---");
            let mut chunk_ids = store
                .latest_at_relevant_chunks_for_all_components(
                    &LatestAtQuery::new(timeline_frame_nr, frame_nr),
                    &entity_path,
                    false, /* don't include static data */
                )
                .to_iter()
                .unwrap()
                .map(|chunk| {
                    eprintln!("{chunk}");
                    chunk.id()
                })
                .collect_vec();
            chunk_ids.sort();

            expected_chunk_ids.sort();

            similar_asserts::assert_eq!(expected_chunk_ids, chunk_ids);
        };

        assert_latest_chunk(frame0, vec![]);
        assert_latest_chunk(frame1, vec![chunk1.id()]);
        assert_latest_chunk(frame2, vec![chunk1.id(), chunk2.id()]); // overlap
        assert_latest_chunk(frame3, vec![chunk1.id(), chunk2.id()]); // overlap
        assert_latest_chunk(frame4, vec![chunk1.id(), chunk2.id()]); // overlap
    }

    Ok(())
}

#[test]
fn latest_at_overlapped_chunks() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let entity_path = EntityPath::from("this/that");

    let frame0 = TimeInt::new_temporal(0);
    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);
    let frame5 = TimeInt::new_temporal(5);
    let frame6 = TimeInt::new_temporal(6);
    let frame7 = TimeInt::new_temporal(7);
    let frame8 = TimeInt::new_temporal(8);

    let points1 = MyPoint::from_iter(0..1);
    let points2 = MyPoint::from_iter(1..2);
    let points3 = MyPoint::from_iter(2..3);
    let points4 = MyPoint::from_iter(3..4);
    let points5 = MyPoint::from_iter(4..5);
    let points6 = MyPoint::from_iter(5..6);
    let points7 = MyPoint::from_iter(6..7);

    let row_id1_1 = RowId::new();
    let row_id1_3 = RowId::new();
    let row_id1_5 = RowId::new();
    let row_id1_7 = RowId::new();
    let chunk1 = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id1_1,
            [build_frame_nr(frame1)],
            [(MyPoints::descriptor_points(), Some(&points1 as _))],
        )
        .with_sparse_component_batches(
            row_id1_3,
            [build_frame_nr(frame3)],
            [(MyPoints::descriptor_points(), Some(&points3 as _))],
        )
        .with_sparse_component_batches(
            row_id1_5,
            [build_frame_nr(frame5)],
            [(MyPoints::descriptor_points(), Some(&points5 as _))],
        )
        .with_sparse_component_batches(
            row_id1_7,
            [build_frame_nr(frame7)],
            [(MyPoints::descriptor_points(), Some(&points7 as _))],
        )
        .build()?;

    let chunk1 = Arc::new(chunk1);
    store.insert_chunk(&chunk1)?;

    let row_id2_2 = RowId::new();
    let row_id2_3 = RowId::new();
    let row_id2_4 = RowId::new();
    let chunk2 = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id2_2,
            [build_frame_nr(frame2)],
            [(MyPoints::descriptor_points(), Some(&points2 as _))],
        )
        .with_sparse_component_batches(
            row_id2_3,
            [build_frame_nr(frame3)],
            [(MyPoints::descriptor_points(), Some(&points3 as _))],
        )
        .with_sparse_component_batches(
            row_id2_4,
            [build_frame_nr(frame4)],
            [(MyPoints::descriptor_points(), Some(&points4 as _))],
        )
        .build()?;

    let chunk2 = Arc::new(chunk2);
    store.insert_chunk(&chunk2)?;

    let row_id3_2 = RowId::new();
    let row_id3_4 = RowId::new();
    let row_id3_6 = RowId::new();
    let chunk3 = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id3_2,
            [build_frame_nr(frame2)],
            [(MyPoints::descriptor_points(), Some(&points2 as _))],
        )
        .with_sparse_component_batches(
            row_id3_4,
            [build_frame_nr(frame4)],
            [(MyPoints::descriptor_points(), Some(&points4 as _))],
        )
        .with_sparse_component_batches(
            row_id3_6,
            [build_frame_nr(frame6)],
            [(MyPoints::descriptor_points(), Some(&points6 as _))],
        )
        .build()?;

    let chunk3 = Arc::new(chunk3);
    store.insert_chunk(&chunk3)?;

    eprintln!("{store}");

    for (at, expected_row_id) in [
        (frame1, row_id1_1),       //
        (frame2, row_id3_2),       //
        (frame3, row_id2_3),       //
        (frame4, row_id3_4),       //
        (frame5, row_id1_5),       //
        (frame6, row_id3_6),       //
        (frame7, row_id1_7),       //
        (TimeInt::MAX, row_id1_7), //
    ] {
        let query = LatestAtQuery::new(TimelineName::new("frame_nr"), at);
        eprintln!("{} @ {query:?}", MyPoints::descriptor_points());
        let row_id =
            query_latest_array(&store, &entity_path, &MyPoints::descriptor_points(), &query)
                .map(|(_data_time, row_id, _array)| row_id);
        assert_eq!(expected_row_id, row_id.unwrap());
    }

    // Component-less APIs
    {
        let assert_latest_chunk = |frame_nr: TimeInt, mut expected_chunk_ids: Vec<ChunkId>| {
            let timeline_frame_nr = TimelineName::new("frame_nr");

            eprintln!("--- {frame_nr:?} ---");
            let mut chunk_ids = store
                .latest_at_relevant_chunks_for_all_components(
                    &LatestAtQuery::new(timeline_frame_nr, frame_nr),
                    &entity_path,
                    false, /* don't include static data */
                )
                .to_iter()
                .unwrap()
                .map(|chunk| {
                    eprintln!("{chunk}");
                    chunk.id()
                })
                .collect_vec();
            chunk_ids.sort();

            expected_chunk_ids.sort();

            similar_asserts::assert_eq!(expected_chunk_ids, chunk_ids);
        };

        assert_latest_chunk(frame0, vec![]);
        assert_latest_chunk(frame1, vec![chunk1.id()]);
        assert_latest_chunk(frame2, vec![chunk1.id(), chunk2.id(), chunk3.id()]); // overlap
        assert_latest_chunk(frame3, vec![chunk1.id(), chunk2.id(), chunk3.id()]); // overlap
        assert_latest_chunk(frame4, vec![chunk1.id(), chunk2.id(), chunk3.id()]); // overlap
        assert_latest_chunk(frame5, vec![chunk1.id(), chunk2.id(), chunk3.id()]); // overlap
        assert_latest_chunk(frame6, vec![chunk1.id(), chunk2.id(), chunk3.id()]); // overlap
        assert_latest_chunk(frame7, vec![chunk1.id(), chunk2.id(), chunk3.id()]); // overlap
        assert_latest_chunk(frame8, vec![chunk1.id(), chunk2.id(), chunk3.id()]);
    }

    Ok(())
}

// ---

#[test]
fn range() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let entity_path = EntityPath::from("this/that");

    let frame0 = TimeInt::new_temporal(0);
    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);
    let frame5 = TimeInt::new_temporal(5);
    let frame6 = TimeInt::new_temporal(6);

    let row_id1 = RowId::new();
    let indices1 = MyIndex::from_iter(0..3);
    let colors1 = MyColor::from_iter(0..3);
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

    let row_id3 = RowId::new();
    let points3 = MyPoint::from_iter(0..10);
    let chunk3 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id3,
                [build_frame_nr(frame3)],
                [(MyPoints::descriptor_points(), &points3 as _)],
            )
            .build()?,
    );

    let row_id4_1 = RowId::new();
    let indices4_1 = MyIndex::from_iter(20..25);
    let colors4_1 = MyColor::from_iter(0..5);
    let chunk4_1 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id4_1,
                [build_frame_nr(frame4)],
                [
                    (MyIndex::partial_descriptor(), &indices4_1 as _),
                    (MyPoints::descriptor_colors(), &colors4_1 as _),
                ],
            )
            .build()?,
    );

    let row_id4_2 = RowId::new();
    let indices4_2 = MyIndex::from_iter(25..30);
    let colors4_2 = MyColor::from_iter(0..5);
    let chunk4_2 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id4_2,
                [build_frame_nr(frame4)],
                [
                    (MyIndex::partial_descriptor(), &indices4_2 as _),
                    (MyPoints::descriptor_colors(), &colors4_2 as _),
                ],
            )
            .build()?,
    );

    let row_id4_25 = RowId::new();
    let points4_25 = MyPoint::from_iter(0..5);
    let chunk4_25 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id4_25,
                [build_frame_nr(frame4)],
                [
                    (MyIndex::partial_descriptor(), &indices4_2 as _),
                    (MyPoints::descriptor_points(), &points4_25 as _),
                ],
            )
            .build()?,
    );

    let row_id4_3 = RowId::new();
    let indices4_3 = MyIndex::from_iter(30..35);
    let colors4_3 = MyColor::from_iter(0..5);
    let chunk4_3 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id4_3,
                [build_frame_nr(frame4)],
                [
                    (MyIndex::partial_descriptor(), &indices4_3 as _),
                    (MyPoints::descriptor_colors(), &colors4_3 as _),
                ],
            )
            .build()?,
    );

    let row_id4_4 = RowId::new();
    let points4_4 = MyPoint::from_iter(0..5);
    let chunk4_4 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id4_4,
                [build_frame_nr(frame4)],
                [
                    (MyIndex::partial_descriptor(), &indices4_3 as _),
                    (MyPoints::descriptor_points(), &points4_4 as _),
                ],
            )
            .build()?,
    );

    // injecting some static colors
    let row_id5 = RowId::new();
    let colors5 = MyColor::from_iter(0..8);
    let chunk5 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(
                row_id5,
                TimePoint::default(),
                [(MyPoints::descriptor_colors(), &colors5 as _)],
            )
            .build()?,
    );

    store.insert_chunk(&chunk1)?;
    store.insert_chunk(&chunk2)?;
    store.insert_chunk(&chunk3)?;
    store.insert_chunk(&chunk4_1)?;
    store.insert_chunk(&chunk4_2)?;
    store.insert_chunk(&chunk4_25)?;
    store.insert_chunk(&chunk4_3)?;
    store.insert_chunk(&chunk4_4)?;
    store.insert_chunk(&chunk5)?;

    // Each entry in `rows_at_times` corresponds to a dataframe that's expected to be returned
    // by the range query.
    // A single timepoint might have several of those! That's one of the behaviors specific to
    // range queries.
    let assert_range_components =
        |time_range: AbsoluteTimeRange,
         component_descr: ComponentDescriptor,
         row_ids_at_times: &[(TimeInt, RowId)]| {
            let timeline_frame_nr = TimelineName::new("frame_nr");

            let query = RangeQuery::new(timeline_frame_nr, time_range);
            let results =
                store.range_relevant_chunks(&query, &entity_path, component_descr.component);

            eprintln!("================= {component_descr} @ {query:?} ===============");
            let mut results_processed = 0usize;
            for chunk in results.to_iter().unwrap() {
                let chunk = chunk.range(&query, component_descr.component);
                eprintln!("{chunk}");
                for (data_time, row_id) in chunk.iter_indices(&timeline_frame_nr) {
                    let (expected_data_time, expected_row_id) = row_ids_at_times[results_processed];
                    assert_eq!(expected_data_time, data_time);
                    assert_eq!(expected_row_id, row_id);

                    results_processed += 1;
                }
            }

            let results_processed_expected = row_ids_at_times.len();
            assert_eq!(results_processed_expected, results_processed);
        };

    // Unit ranges

    assert_range_components(
        AbsoluteTimeRange::new(frame1, frame1),
        MyPoints::descriptor_colors(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        AbsoluteTimeRange::new(frame1, frame1),
        MyPoints::descriptor_points(),
        &[],
    );
    assert_range_components(
        AbsoluteTimeRange::new(frame2, frame2),
        MyPoints::descriptor_colors(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        AbsoluteTimeRange::new(frame2, frame2),
        MyPoints::descriptor_points(),
        &[(frame2, row_id2)],
    );
    assert_range_components(
        AbsoluteTimeRange::new(frame3, frame3),
        MyPoints::descriptor_colors(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        AbsoluteTimeRange::new(frame3, frame3),
        MyPoints::descriptor_points(),
        &[(frame3, row_id3)],
    );
    assert_range_components(
        AbsoluteTimeRange::new(frame4, frame4),
        MyPoints::descriptor_colors(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        AbsoluteTimeRange::new(frame4, frame4),
        MyPoints::descriptor_points(),
        &[(frame4, row_id4_25), (frame4, row_id4_4)],
    );
    assert_range_components(
        AbsoluteTimeRange::new(frame5, frame5),
        MyPoints::descriptor_colors(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        AbsoluteTimeRange::new(frame5, frame5),
        MyPoints::descriptor_points(),
        &[],
    );

    // Full range

    assert_range_components(
        AbsoluteTimeRange::new(frame1, frame5),
        MyPoints::descriptor_points(),
        &[
            (frame2, row_id2),
            (frame3, row_id3),
            (frame4, row_id4_25),
            (frame4, row_id4_4),
        ],
    );
    assert_range_components(
        AbsoluteTimeRange::new(frame1, frame5),
        MyPoints::descriptor_colors(),
        &[(TimeInt::STATIC, row_id5)],
    );

    // Infinite range

    assert_range_components(
        AbsoluteTimeRange::new(TimeInt::MIN, TimeInt::MAX),
        MyPoints::descriptor_points(),
        &[
            (frame2, row_id2),
            (frame3, row_id3),
            (frame4, row_id4_25),
            (frame4, row_id4_4),
        ],
    );
    assert_range_components(
        AbsoluteTimeRange::new(TimeInt::MIN, TimeInt::MAX),
        MyPoints::descriptor_colors(),
        &[(TimeInt::STATIC, row_id5)],
    );

    // Component-less APIs
    {
        let assert_range_chunk =
            |time_range: AbsoluteTimeRange, mut expected_chunk_ids: Vec<ChunkId>| {
                let timeline_frame_nr = TimelineName::new("frame_nr");

                eprintln!("--- {time_range:?} ---");
                let mut chunk_ids = store
                    .range_relevant_chunks_for_all_components(
                        &RangeQuery::new(timeline_frame_nr, time_range),
                        &entity_path,
                        false, /* don't include static data */
                    )
                    .to_iter()
                    .unwrap()
                    .map(|chunk| {
                        eprintln!("{chunk}");
                        chunk.id()
                    })
                    .collect_vec();
                chunk_ids.sort();

                expected_chunk_ids.sort();

                similar_asserts::assert_eq!(expected_chunk_ids, chunk_ids);
            };

        // Unit ranges
        assert_range_chunk(AbsoluteTimeRange::new(frame0, frame0), vec![]);
        assert_range_chunk(AbsoluteTimeRange::new(frame1, frame1), vec![chunk1.id()]);
        assert_range_chunk(AbsoluteTimeRange::new(frame2, frame2), vec![chunk2.id()]);
        assert_range_chunk(AbsoluteTimeRange::new(frame3, frame3), vec![chunk3.id()]);
        assert_range_chunk(
            AbsoluteTimeRange::new(frame4, frame4),
            vec![
                chunk4_1.id(),
                chunk4_2.id(),
                chunk4_25.id(),
                chunk4_3.id(),
                chunk4_4.id(),
            ],
        );
        assert_range_chunk(AbsoluteTimeRange::new(frame5, frame5), vec![]);
        assert_range_chunk(AbsoluteTimeRange::new(frame6, frame6), vec![]);

        // Full range
        assert_range_chunk(
            AbsoluteTimeRange::new(frame1, frame5),
            vec![
                chunk1.id(),
                chunk2.id(),
                chunk3.id(),
                chunk4_1.id(),
                chunk4_2.id(),
                chunk4_25.id(),
                chunk4_3.id(),
                chunk4_4.id(),
            ],
        );

        // Infinite range
        assert_range_chunk(
            AbsoluteTimeRange::EVERYTHING,
            vec![
                chunk1.id(),
                chunk2.id(),
                chunk3.id(),
                chunk4_1.id(),
                chunk4_2.id(),
                chunk4_25.id(),
                chunk4_3.id(),
                chunk4_4.id(),
            ],
        );
    }

    Ok(())
}

#[test]
fn range_overlapped_chunks() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording, "test_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let entity_path = EntityPath::from("this/that");

    let frame0 = TimeInt::new_temporal(0);
    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);
    let frame5 = TimeInt::new_temporal(5);
    let frame6 = TimeInt::new_temporal(6);
    let frame7 = TimeInt::new_temporal(7);
    let frame8 = TimeInt::new_temporal(8);

    let points1 = MyPoint::from_iter(0..1);
    let points2 = MyPoint::from_iter(1..2);
    let points3 = MyPoint::from_iter(2..3);
    let points4 = MyPoint::from_iter(3..4);
    let points5 = MyPoint::from_iter(4..5);
    let points7_1 = MyPoint::from_iter(6..7);
    let points7_2 = MyPoint::from_iter(7..8);
    let points7_3 = MyPoint::from_iter(8..9);

    let row_id1_1 = RowId::new();
    let row_id1_3 = RowId::new();
    let row_id1_5 = RowId::new();
    let row_id1_7_1 = RowId::new();
    let row_id1_7_2 = RowId::new();
    let row_id1_7_3 = RowId::new();
    let chunk1_1 = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id1_1,
            [build_frame_nr(frame1)],
            [(MyPoints::descriptor_points(), Some(&points1 as _))],
        )
        .with_sparse_component_batches(
            row_id1_3,
            [build_frame_nr(frame3)],
            [(MyPoints::descriptor_points(), Some(&points3 as _))],
        )
        .with_sparse_component_batches(
            row_id1_5,
            [build_frame_nr(frame5)],
            [(MyPoints::descriptor_points(), Some(&points5 as _))],
        )
        .with_sparse_component_batches(
            row_id1_7_1,
            [build_frame_nr(frame7)],
            [(MyPoints::descriptor_points(), Some(&points7_1 as _))],
        )
        .with_sparse_component_batches(
            row_id1_7_2,
            [build_frame_nr(frame7)],
            [(MyPoints::descriptor_points(), Some(&points7_2 as _))],
        )
        .with_sparse_component_batches(
            row_id1_7_3,
            [build_frame_nr(frame7)],
            [(MyPoints::descriptor_points(), Some(&points7_3 as _))],
        )
        .build()?;

    let chunk1_1 = Arc::new(chunk1_1);
    store.insert_chunk(&chunk1_1)?;
    let chunk1_2 = Arc::new(chunk1_1.clone_as(ChunkId::new(), RowId::new()));
    store.insert_chunk(&chunk1_2)?; // x2 !
    let chunk1_3 = Arc::new(chunk1_1.clone_as(ChunkId::new(), RowId::new()));
    store.insert_chunk(&chunk1_3)?; // x3 !!

    let row_id2_2 = RowId::new();
    let row_id2_3 = RowId::new();
    let row_id2_4 = RowId::new();
    let chunk2 = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id2_2,
            [build_frame_nr(frame2)],
            [(MyPoints::descriptor_points(), Some(&points2 as _))],
        )
        .with_sparse_component_batches(
            row_id2_3,
            [build_frame_nr(frame3)],
            [(MyPoints::descriptor_points(), Some(&points3 as _))],
        )
        .with_sparse_component_batches(
            row_id2_4,
            [build_frame_nr(frame4)],
            [(MyPoints::descriptor_points(), Some(&points4 as _))],
        )
        .build()?;

    let chunk2 = Arc::new(chunk2);
    store.insert_chunk(&chunk2)?;

    let assert_range_chunk = |time_range: AbsoluteTimeRange,
                              mut expected_chunk_ids: Vec<ChunkId>| {
        let timeline_frame_nr = TimelineName::new("frame_nr");

        eprintln!("--- {time_range:?} ---");
        let mut chunk_ids = store
            .range_relevant_chunks_for_all_components(
                &RangeQuery::new(timeline_frame_nr, time_range),
                &entity_path,
                false, /* don't include static data */
            )
            .to_iter()
            .unwrap()
            .map(|chunk| {
                eprintln!("{chunk}");
                chunk.id()
            })
            .collect_vec();
        chunk_ids.sort();

        expected_chunk_ids.sort();

        similar_asserts::assert_eq!(expected_chunk_ids, chunk_ids);
    };

    // Unit ranges
    assert_range_chunk(AbsoluteTimeRange::new(frame0, frame0), vec![]);
    assert_range_chunk(
        AbsoluteTimeRange::new(frame1, frame1),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id()],
    );
    assert_range_chunk(
        AbsoluteTimeRange::new(frame2, frame2),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id(), chunk2.id()],
    );
    assert_range_chunk(
        AbsoluteTimeRange::new(frame3, frame3),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id(), chunk2.id()],
    );
    assert_range_chunk(
        AbsoluteTimeRange::new(frame4, frame4),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id(), chunk2.id()],
    );
    assert_range_chunk(
        AbsoluteTimeRange::new(frame5, frame5),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id()],
    );
    assert_range_chunk(
        AbsoluteTimeRange::new(frame6, frame6),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id()],
    );
    assert_range_chunk(
        AbsoluteTimeRange::new(frame7, frame7),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id()],
    );
    assert_range_chunk(AbsoluteTimeRange::new(frame8, frame8), vec![]);

    // Full range
    assert_range_chunk(
        AbsoluteTimeRange::new(frame1, frame5),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id(), chunk2.id()],
    );

    // Infinite range
    assert_range_chunk(
        AbsoluteTimeRange::EVERYTHING,
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id(), chunk2.id()],
    );

    Ok(())
}
