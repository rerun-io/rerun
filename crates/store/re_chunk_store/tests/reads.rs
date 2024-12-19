use std::sync::Arc;

use arrow2::array::Array as Arrow2Array;

use itertools::Itertools;
use re_chunk::{Chunk, ChunkId, RowId, TimePoint};
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, LatestAtQuery, RangeQuery, ResolvedTimeRange, TimeInt,
};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyIndex, MyPoint},
    EntityPath, TimeType, Timeline,
};
use re_types::{
    testing::{build_some_large_structs, LargeStruct},
    ComponentDescriptor, ComponentNameSet,
};
use re_types_core::Component as _;

// ---

fn query_latest_array(
    store: &ChunkStore,
    entity_path: &EntityPath,
    component_desc: &ComponentDescriptor,
    query: &LatestAtQuery,
) -> Option<(TimeInt, RowId, Box<dyn Arrow2Array>)> {
    re_tracing::profile_function!();

    let ((data_time, row_id), unit) = store
        .latest_at_relevant_chunks(query, entity_path, component_desc.component_name)
        .into_iter()
        .filter_map(|chunk| {
            chunk
                .latest_at(query, component_desc.component_name)
                .into_unit()
                .and_then(|chunk| chunk.index(&query.timeline()).map(|index| (index, chunk)))
        })
        .max_by_key(|(index, _chunk)| *index)?;

    unit.component_batch_raw_arrow2(&component_desc.component_name)
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
            let timeline = Timeline::new("frame_nr", TimeType::Sequence);

            let component_names = store.all_components_on_timeline_sorted(&timeline, entity_path);

            let expected_component_names = expected.map(|expected| {
                let expected: ComponentNameSet =
                    expected.iter().map(|desc| desc.component_name).collect();
                expected
            });

            assert_eq!(
                expected_component_names, component_names,
                "expected to find {expected_component_names:?}, found {component_names:?} instead\n{store}",
            );
        };

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );

    let components_a = &[
        MyColor::descriptor(),     // added by test, static
        LargeStruct::descriptor(), // added by test
    ];

    let components_b = &[
        MyColor::descriptor(),     // added by test, static
        MyPoint::descriptor(),     // added by test
        LargeStruct::descriptor(), // added by test
    ];

    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(
            RowId::new(),
            TimePoint::default(),
            &MyColor::from_iter(0..2),
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batch(
            RowId::new(),
            [build_frame_nr(frame1)],
            &build_some_large_structs(2),
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    assert_latest_components_at(&mut store, &entity_path, Some(components_a));

    let chunk = Chunk::builder(entity_path.clone())
        .with_component_batches(
            RowId::new(),
            [build_frame_nr(frame2)],
            [
                &build_some_large_structs(2) as _,
                &MyPoint::from_iter(0..2) as _,
            ],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    assert_latest_components_at(&mut store, &entity_path, Some(components_b));

    Ok(())
}

// ---

#[test]
fn latest_at() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
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

    // injecting some static colors
    let row_id5 = RowId::new();
    let colors5 = MyColor::from_iter(0..3);
    let chunk5 = Chunk::builder(entity_path.clone())
        .with_component_batches(row_id5, TimePoint::default(), [&colors5 as _])
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
            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

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
            (MyColor::descriptor(), Some(row_id5)), // static
            (MyIndex::descriptor(), None),
            (MyPoint::descriptor(), None),
        ],
    );
    assert_latest_components(
        frame1,
        &[
            (MyColor::descriptor(), Some(row_id5)), // static
            (MyIndex::descriptor(), Some(row_id1)),
            (MyPoint::descriptor(), None),
        ],
    );
    assert_latest_components(
        frame2,
        &[
            (MyColor::descriptor(), Some(row_id5)),
            (MyPoint::descriptor(), Some(row_id2)),
            (MyIndex::descriptor(), Some(row_id2)),
        ],
    );
    assert_latest_components(
        frame3,
        &[
            (MyColor::descriptor(), Some(row_id5)),
            (MyPoint::descriptor(), Some(row_id3)),
            (MyIndex::descriptor(), Some(row_id2)),
        ],
    );
    assert_latest_components(
        frame4,
        &[
            (MyColor::descriptor(), Some(row_id5)),
            (MyPoint::descriptor(), Some(row_id3)),
            (MyIndex::descriptor(), Some(row_id2)),
        ],
    );

    // Component-less APIs
    {
        let assert_latest_chunk =
            |store: &ChunkStore, frame_nr: TimeInt, mut expected_chunk_ids: Vec<ChunkId>| {
                let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

                let mut chunk_ids = store
                    .latest_at_relevant_chunks_for_all_components(
                        &LatestAtQuery::new(timeline_frame_nr, frame_nr),
                        &entity_path,
                    )
                    .into_iter()
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
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
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
                (MyIndex::descriptor(), None),
                (MyPoint::descriptor(), Some(&MyPoint::from_iter(0..1) as _)),
            ],
        )
        .with_sparse_component_batches(
            row_id1_2,
            [build_frame_nr(frame2)],
            [
                (MyIndex::descriptor(), None),
                (MyPoint::descriptor(), Some(&MyPoint::from_iter(1..2) as _)),
            ],
        )
        .with_sparse_component_batches(
            row_id1_3,
            [build_frame_nr(frame3)],
            [
                (MyIndex::descriptor(), Some(&MyIndex::from_iter(2..3) as _)),
                (MyPoint::descriptor(), Some(&MyPoint::from_iter(2..3) as _)),
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
                (MyIndex::descriptor(), Some(&MyIndex::from_iter(2..3) as _)),
                (MyPoint::descriptor(), Some(&MyPoint::from_iter(1..2) as _)),
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
        &MyIndex::descriptor(),
        &LatestAtQuery::new(Timeline::new_sequence("frame_nr"), TimeInt::MAX),
    )
    .map(|(_data_time, row_id, _array)| row_id);

    assert_eq!(row_id1_3, row_id.unwrap());

    // Component-less APIs
    {
        let assert_latest_chunk = |frame_nr: TimeInt, mut expected_chunk_ids: Vec<ChunkId>| {
            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

            eprintln!("--- {frame_nr:?} ---");
            let mut chunk_ids = store
                .latest_at_relevant_chunks_for_all_components(
                    &LatestAtQuery::new(timeline_frame_nr, frame_nr),
                    &entity_path,
                )
                .into_iter()
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
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
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
            [(MyPoint::descriptor(), Some(&points1 as _))],
        )
        .with_sparse_component_batches(
            row_id1_3,
            [build_frame_nr(frame3)],
            [(MyPoint::descriptor(), Some(&points3 as _))],
        )
        .with_sparse_component_batches(
            row_id1_5,
            [build_frame_nr(frame5)],
            [(MyPoint::descriptor(), Some(&points5 as _))],
        )
        .with_sparse_component_batches(
            row_id1_7,
            [build_frame_nr(frame7)],
            [(MyPoint::descriptor(), Some(&points7 as _))],
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
            [(MyPoint::descriptor(), Some(&points2 as _))],
        )
        .with_sparse_component_batches(
            row_id2_3,
            [build_frame_nr(frame3)],
            [(MyPoint::descriptor(), Some(&points3 as _))],
        )
        .with_sparse_component_batches(
            row_id2_4,
            [build_frame_nr(frame4)],
            [(MyPoint::descriptor(), Some(&points4 as _))],
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
            [(MyPoint::descriptor(), Some(&points2 as _))],
        )
        .with_sparse_component_batches(
            row_id3_4,
            [build_frame_nr(frame4)],
            [(MyPoint::descriptor(), Some(&points4 as _))],
        )
        .with_sparse_component_batches(
            row_id3_6,
            [build_frame_nr(frame6)],
            [(MyPoint::descriptor(), Some(&points6 as _))],
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
        let query = LatestAtQuery::new(Timeline::new_sequence("frame_nr"), at);
        eprintln!("{} @ {query:?}", MyPoint::descriptor());
        let row_id = query_latest_array(&store, &entity_path, &MyPoint::descriptor(), &query)
            .map(|(_data_time, row_id, _array)| row_id);
        assert_eq!(expected_row_id, row_id.unwrap());
    }

    // Component-less APIs
    {
        let assert_latest_chunk = |frame_nr: TimeInt, mut expected_chunk_ids: Vec<ChunkId>| {
            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

            eprintln!("--- {frame_nr:?} ---");
            let mut chunk_ids = store
                .latest_at_relevant_chunks_for_all_components(
                    &LatestAtQuery::new(timeline_frame_nr, frame_nr),
                    &entity_path,
                )
                .into_iter()
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
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
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

    let row_id3 = RowId::new();
    let points3 = MyPoint::from_iter(0..10);
    let chunk3 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(row_id3, [build_frame_nr(frame3)], [&points3 as _])
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
                [&indices4_1 as _, &colors4_1 as _],
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
                [&indices4_2 as _, &colors4_2 as _],
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
                [&indices4_2 as _, &points4_25 as _],
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
                [&indices4_3 as _, &colors4_3 as _],
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
                [&indices4_3 as _, &points4_4 as _],
            )
            .build()?,
    );

    // injecting some static colors
    let row_id5 = RowId::new();
    let colors5 = MyColor::from_iter(0..8);
    let chunk5 = Arc::new(
        Chunk::builder(entity_path.clone())
            .with_component_batches(row_id5, TimePoint::default(), [&colors5 as _])
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
    #[allow(clippy::type_complexity)]
    let assert_range_components =
        |time_range: ResolvedTimeRange,
         component_desc: ComponentDescriptor,
         row_ids_at_times: &[(TimeInt, RowId)]| {
            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

            let query = RangeQuery::new(timeline_frame_nr, time_range);
            let results =
                store.range_relevant_chunks(&query, &entity_path, component_desc.component_name);

            eprintln!("================= {component_desc} @ {query:?} ===============");
            let mut results_processed = 0usize;
            for chunk in results {
                let chunk = chunk.range(&query, component_desc.component_name);
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
        ResolvedTimeRange::new(frame1, frame1),
        MyColor::descriptor(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame1, frame1),
        MyPoint::descriptor(),
        &[],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame2, frame2),
        MyColor::descriptor(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame2, frame2),
        MyPoint::descriptor(),
        &[(frame2, row_id2)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame3, frame3),
        MyColor::descriptor(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame3, frame3),
        MyPoint::descriptor(),
        &[(frame3, row_id3)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame4, frame4),
        MyColor::descriptor(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame4, frame4),
        MyPoint::descriptor(),
        &[(frame4, row_id4_25), (frame4, row_id4_4)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame5, frame5),
        MyColor::descriptor(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame5, frame5),
        MyPoint::descriptor(),
        &[],
    );

    // Full range

    assert_range_components(
        ResolvedTimeRange::new(frame1, frame5),
        MyPoint::descriptor(),
        &[
            (frame2, row_id2),
            (frame3, row_id3),
            (frame4, row_id4_25),
            (frame4, row_id4_4),
        ],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame1, frame5),
        MyColor::descriptor(),
        &[(TimeInt::STATIC, row_id5)],
    );

    // Infinite range

    assert_range_components(
        ResolvedTimeRange::new(TimeInt::MIN, TimeInt::MAX),
        MyPoint::descriptor(),
        &[
            (frame2, row_id2),
            (frame3, row_id3),
            (frame4, row_id4_25),
            (frame4, row_id4_4),
        ],
    );
    assert_range_components(
        ResolvedTimeRange::new(TimeInt::MIN, TimeInt::MAX),
        MyColor::descriptor(),
        &[(TimeInt::STATIC, row_id5)],
    );

    // Component-less APIs
    {
        let assert_range_chunk =
            |time_range: ResolvedTimeRange, mut expected_chunk_ids: Vec<ChunkId>| {
                let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

                eprintln!("--- {time_range:?} ---");
                let mut chunk_ids = store
                    .range_relevant_chunks_for_all_components(
                        &RangeQuery::new(timeline_frame_nr, time_range),
                        &entity_path,
                    )
                    .into_iter()
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
        assert_range_chunk(ResolvedTimeRange::new(frame0, frame0), vec![]);
        assert_range_chunk(ResolvedTimeRange::new(frame1, frame1), vec![chunk1.id()]);
        assert_range_chunk(ResolvedTimeRange::new(frame2, frame2), vec![chunk2.id()]);
        assert_range_chunk(ResolvedTimeRange::new(frame3, frame3), vec![chunk3.id()]);
        assert_range_chunk(
            ResolvedTimeRange::new(frame4, frame4),
            vec![
                chunk4_1.id(),
                chunk4_2.id(),
                chunk4_25.id(),
                chunk4_3.id(),
                chunk4_4.id(),
            ],
        );
        assert_range_chunk(ResolvedTimeRange::new(frame5, frame5), vec![]);
        assert_range_chunk(ResolvedTimeRange::new(frame6, frame6), vec![]);

        // Full range
        assert_range_chunk(
            ResolvedTimeRange::new(frame1, frame5),
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
            ResolvedTimeRange::EVERYTHING,
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
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
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
            [(MyPoint::descriptor(), Some(&points1 as _))],
        )
        .with_sparse_component_batches(
            row_id1_3,
            [build_frame_nr(frame3)],
            [(MyPoint::descriptor(), Some(&points3 as _))],
        )
        .with_sparse_component_batches(
            row_id1_5,
            [build_frame_nr(frame5)],
            [(MyPoint::descriptor(), Some(&points5 as _))],
        )
        .with_sparse_component_batches(
            row_id1_7_1,
            [build_frame_nr(frame7)],
            [(MyPoint::descriptor(), Some(&points7_1 as _))],
        )
        .with_sparse_component_batches(
            row_id1_7_2,
            [build_frame_nr(frame7)],
            [(MyPoint::descriptor(), Some(&points7_2 as _))],
        )
        .with_sparse_component_batches(
            row_id1_7_3,
            [build_frame_nr(frame7)],
            [(MyPoint::descriptor(), Some(&points7_3 as _))],
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
            [(MyPoint::descriptor(), Some(&points2 as _))],
        )
        .with_sparse_component_batches(
            row_id2_3,
            [build_frame_nr(frame3)],
            [(MyPoint::descriptor(), Some(&points3 as _))],
        )
        .with_sparse_component_batches(
            row_id2_4,
            [build_frame_nr(frame4)],
            [(MyPoint::descriptor(), Some(&points4 as _))],
        )
        .build()?;

    let chunk2 = Arc::new(chunk2);
    store.insert_chunk(&chunk2)?;

    let assert_range_chunk = |time_range: ResolvedTimeRange,
                              mut expected_chunk_ids: Vec<ChunkId>| {
        let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

        eprintln!("--- {time_range:?} ---");
        let mut chunk_ids = store
            .range_relevant_chunks_for_all_components(
                &RangeQuery::new(timeline_frame_nr, time_range),
                &entity_path,
            )
            .into_iter()
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
    assert_range_chunk(ResolvedTimeRange::new(frame0, frame0), vec![]);
    assert_range_chunk(
        ResolvedTimeRange::new(frame1, frame1),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id()],
    );
    assert_range_chunk(
        ResolvedTimeRange::new(frame2, frame2),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id(), chunk2.id()],
    );
    assert_range_chunk(
        ResolvedTimeRange::new(frame3, frame3),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id(), chunk2.id()],
    );
    assert_range_chunk(
        ResolvedTimeRange::new(frame4, frame4),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id(), chunk2.id()],
    );
    assert_range_chunk(
        ResolvedTimeRange::new(frame5, frame5),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id()],
    );
    assert_range_chunk(
        ResolvedTimeRange::new(frame6, frame6),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id()],
    );
    assert_range_chunk(
        ResolvedTimeRange::new(frame7, frame7),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id()],
    );
    assert_range_chunk(ResolvedTimeRange::new(frame8, frame8), vec![]);

    // Full range
    assert_range_chunk(
        ResolvedTimeRange::new(frame1, frame5),
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id(), chunk2.id()],
    );

    // Infinite range
    assert_range_chunk(
        ResolvedTimeRange::EVERYTHING,
        vec![chunk1_1.id(), chunk1_2.id(), chunk1_3.id(), chunk2.id()],
    );

    Ok(())
}
