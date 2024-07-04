use std::sync::Arc;

use arrow2::array::Array as ArrowArray;

use itertools::Itertools;
use re_chunk::{Chunk, RowId, TimePoint};
use re_chunk_store::{
    ChunkStore, ChunkStoreConfig, LatestAtQuery, RangeQuery, ResolvedTimeRange, TimeInt,
};
use re_log_types::{
    build_frame_nr,
    example_components::{MyColor, MyIndex, MyPoint},
    EntityPath, TimeType, Timeline,
};
use re_types::testing::{build_some_large_structs, LargeStruct};
use re_types::ComponentNameSet;
use re_types_core::{ComponentName, Loggable as _};

// ---

fn query_latest_array(
    store: &ChunkStore,
    entity_path: &EntityPath,
    component_name: ComponentName,
    query: &LatestAtQuery,
) -> Option<(TimeInt, RowId, Box<dyn ArrowArray>)> {
    re_tracing::profile_function!();

    let (data_time, row_id, array) = store
        .latest_at_relevant_chunks(query, entity_path, component_name)
        .into_iter()
        .flat_map(|chunk| {
            chunk
                .latest_at(query, component_name)
                .iter_rows(&query.timeline(), &component_name)
                .collect_vec()
        })
        .max_by_key(|(data_time, row_id, _)| (*data_time, *row_id))
        .and_then(|(data_time, row_id, array)| array.map(|array| (data_time, row_id, array)))?;

    Some((data_time, row_id, array))
}

// ---

#[test]
fn all_components() -> anyhow::Result<()> {
    re_log::setup_logging();

    let entity_path = EntityPath::from("this/that");

    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);

    let assert_latest_components_at =
        |store: &ChunkStore, entity_path: &EntityPath, expected: Option<&[ComponentName]>| {
            let timeline = Timeline::new("frame_nr", TimeType::Sequence);

            let component_names = store.all_components(&timeline, entity_path);

            let expected_component_names = expected.map(|expected| {
                let expected: ComponentNameSet = expected.iter().copied().collect();
                expected
            });

            assert_eq!(
                expected_component_names, component_names,
                "expected to find {expected_component_names:?}, found {component_names:?} instead\n{store}",
            );
        };

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        ChunkStoreConfig::default(),
    );

    let components_a = &[
        MyColor::name(),     // added by test, static
        LargeStruct::name(), // added by test
    ];

    let components_b = &[
        MyColor::name(),     // added by test, static
        MyPoint::name(),     // added by test
        LargeStruct::name(), // added by test
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
        ChunkStoreConfig::default(),
    );

    let entity_path = EntityPath::from("this/that");

    let frame0 = TimeInt::new_temporal(0);
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

    // injecting some static colors
    let row_id5 = RowId::new();
    let colors5 = MyColor::from_iter(0..3);
    let chunk5 = Chunk::builder(entity_path.clone())
        .with_component_batches(row_id5, TimePoint::default(), [&colors5 as _])
        .build()?;

    store.insert_chunk(&Arc::new(chunk1))?;
    store.insert_chunk(&Arc::new(chunk2))?;
    store.insert_chunk(&Arc::new(chunk3))?;
    store.insert_chunk(&Arc::new(chunk4))?;
    store.insert_chunk(&Arc::new(chunk5))?;

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

    assert_latest_components(
        frame0,
        &[
            (MyColor::name(), Some(row_id5)), // static
            (MyIndex::name(), None),
            (MyPoint::name(), None),
        ],
    );
    assert_latest_components(
        frame1,
        &[
            (MyColor::name(), Some(row_id5)), // static
            (MyIndex::name(), Some(row_id1)),
            (MyPoint::name(), None),
        ],
    );
    assert_latest_components(
        frame2,
        &[
            (MyColor::name(), Some(row_id5)),
            (MyPoint::name(), Some(row_id2)),
            (MyIndex::name(), Some(row_id2)),
        ],
    );
    assert_latest_components(
        frame3,
        &[
            (MyColor::name(), Some(row_id5)),
            (MyPoint::name(), Some(row_id3)),
            (MyIndex::name(), Some(row_id2)),
        ],
    );
    assert_latest_components(
        frame4,
        &[
            (MyColor::name(), Some(row_id5)),
            (MyPoint::name(), Some(row_id3)),
            (MyIndex::name(), Some(row_id2)),
        ],
    );

    Ok(())
}

#[test]
fn latest_at_sparse_component_edge_case() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        ChunkStoreConfig::default(),
    );

    let entity_path = EntityPath::from("this/that");

    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);

    // This chunk has a time range of `(1, 3)`, but the actual data for `MyIndex` actually only
    // starts at `3`.

    let row_id1_1 = RowId::new();
    let row_id1_2 = RowId::new();
    let row_id1_3 = RowId::new();
    let chunk = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id1_1,
            [build_frame_nr(frame1)],
            [
                (MyIndex::name(), None),
                (MyPoint::name(), Some(&MyPoint::from_iter(0..1) as _)),
            ],
        )
        .with_sparse_component_batches(
            row_id1_2,
            [build_frame_nr(frame2)],
            [
                (MyIndex::name(), None),
                (MyPoint::name(), Some(&MyPoint::from_iter(1..2) as _)),
            ],
        )
        .with_sparse_component_batches(
            row_id1_3,
            [build_frame_nr(frame3)],
            [
                (MyIndex::name(), Some(&MyIndex::from_iter(2..3) as _)),
                (MyPoint::name(), Some(&MyPoint::from_iter(2..3) as _)),
            ],
        )
        .build()?;
    eprintln!("chunk 1:\n{chunk}");
    store.insert_chunk(&Arc::new(chunk))?;

    // This chunk on the other hand has a time range of `(2, 3)`, and the data for `MyIndex`
    // actually does start at `2`.

    let row_id2_1 = RowId::new();
    let chunk = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id2_1,
            [build_frame_nr(frame2)],
            [
                (MyIndex::name(), Some(&MyIndex::from_iter(2..3) as _)),
                (MyPoint::name(), Some(&MyPoint::from_iter(1..2) as _)),
            ],
        )
        .build()?;
    eprintln!("chunk 2:\n{chunk}");
    store.insert_chunk(&Arc::new(chunk))?;

    // We expect the data for `MyIndex` to come from `row_id_1_3`, since it is the most recent
    // piece of data.
    // The only way this can happen is if we have proper per-component time-ranges, since a global
    // per-chunk time-range would erroneously push us towards the second chunk.

    let row_id = query_latest_array(
        &store,
        &entity_path,
        MyIndex::name(),
        &LatestAtQuery::new(Timeline::new_sequence("frame_nr"), TimeInt::MAX),
    )
    .map(|(_data_time, row_id, _array)| row_id);

    assert_eq!(row_id1_3, row_id.unwrap());

    Ok(())
}

#[test]
fn latest_at_overlapped_chunks() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        ChunkStoreConfig::default(),
    );

    let entity_path = EntityPath::from("this/that");

    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);
    let frame5 = TimeInt::new_temporal(5);
    let frame6 = TimeInt::new_temporal(6);
    let frame7 = TimeInt::new_temporal(7);

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
    let chunk = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id1_1,
            [build_frame_nr(frame1)],
            [(MyPoint::name(), Some(&points1 as _))],
        )
        .with_sparse_component_batches(
            row_id1_3,
            [build_frame_nr(frame3)],
            [(MyPoint::name(), Some(&points3 as _))],
        )
        .with_sparse_component_batches(
            row_id1_5,
            [build_frame_nr(frame5)],
            [(MyPoint::name(), Some(&points5 as _))],
        )
        .with_sparse_component_batches(
            row_id1_7,
            [build_frame_nr(frame7)],
            [(MyPoint::name(), Some(&points7 as _))],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    let row_id2_2 = RowId::new();
    let row_id2_3 = RowId::new();
    let row_id2_4 = RowId::new();
    let chunk = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id2_2,
            [build_frame_nr(frame2)],
            [(MyPoint::name(), Some(&points2 as _))],
        )
        .with_sparse_component_batches(
            row_id2_3,
            [build_frame_nr(frame3)],
            [(MyPoint::name(), Some(&points3 as _))],
        )
        .with_sparse_component_batches(
            row_id2_4,
            [build_frame_nr(frame4)],
            [(MyPoint::name(), Some(&points4 as _))],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

    let row_id3_2 = RowId::new();
    let row_id3_4 = RowId::new();
    let row_id3_6 = RowId::new();
    let chunk = Chunk::builder(entity_path.clone())
        .with_sparse_component_batches(
            row_id3_2,
            [build_frame_nr(frame2)],
            [(MyPoint::name(), Some(&points2 as _))],
        )
        .with_sparse_component_batches(
            row_id3_4,
            [build_frame_nr(frame4)],
            [(MyPoint::name(), Some(&points4 as _))],
        )
        .with_sparse_component_batches(
            row_id3_6,
            [build_frame_nr(frame6)],
            [(MyPoint::name(), Some(&points6 as _))],
        )
        .build()?;
    store.insert_chunk(&Arc::new(chunk))?;

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
        eprintln!("{} @ {query:?}", MyPoint::name());
        let row_id = query_latest_array(&store, &entity_path, MyPoint::name(), &query)
            .map(|(_data_time, row_id, _array)| row_id);
        assert_eq!(expected_row_id, row_id.unwrap());
    }

    Ok(())
}

// ---

#[test]
fn range() -> anyhow::Result<()> {
    re_log::setup_logging();

    let mut store = ChunkStore::new(
        re_log_types::StoreId::random(re_log_types::StoreKind::Recording),
        ChunkStoreConfig::default(),
    );

    let entity_path = EntityPath::from("this/that");

    let frame1 = TimeInt::new_temporal(1);
    let frame2 = TimeInt::new_temporal(2);
    let frame3 = TimeInt::new_temporal(3);
    let frame4 = TimeInt::new_temporal(4);
    let frame5 = TimeInt::new_temporal(5);

    let row_id1 = RowId::new();
    let indices1 = MyIndex::from_iter(0..3);
    let colors1 = MyColor::from_iter(0..3);
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

    let row_id4_1 = RowId::new();
    let indices4_1 = MyIndex::from_iter(20..25);
    let colors4_1 = MyColor::from_iter(0..5);
    let chunk4_1 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id4_1,
            [build_frame_nr(frame4)],
            [&indices4_1 as _, &colors4_1 as _],
        )
        .build()?;

    let row_id4_2 = RowId::new();
    let indices4_2 = MyIndex::from_iter(25..30);
    let colors4_2 = MyColor::from_iter(0..5);
    let chunk4_2 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id4_2,
            [build_frame_nr(frame4)],
            [&indices4_2 as _, &colors4_2 as _],
        )
        .build()?;

    let row_id4_25 = RowId::new();
    let points4_25 = MyPoint::from_iter(0..5);
    let chunk4_25 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id4_25,
            [build_frame_nr(frame4)],
            [&indices4_2 as _, &points4_25 as _],
        )
        .build()?;

    let row_id4_3 = RowId::new();
    let indices4_3 = MyIndex::from_iter(30..35);
    let colors4_3 = MyColor::from_iter(0..5);
    let chunk4_3 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id4_3,
            [build_frame_nr(frame4)],
            [&indices4_3 as _, &colors4_3 as _],
        )
        .build()?;

    let row_id4_4 = RowId::new();
    let points4_4 = MyPoint::from_iter(0..5);
    let chunk4_4 = Chunk::builder(entity_path.clone())
        .with_component_batches(
            row_id4_4,
            [build_frame_nr(frame4)],
            [&indices4_3 as _, &points4_4 as _],
        )
        .build()?;

    // injecting some static colors
    let row_id5 = RowId::new();
    let colors5 = MyColor::from_iter(0..8);
    let chunk5 = Chunk::builder(entity_path.clone())
        .with_component_batches(row_id5, TimePoint::default(), [&colors5 as _])
        .build()?;

    store.insert_chunk(&Arc::new(chunk1))?;
    store.insert_chunk(&Arc::new(chunk2))?;
    store.insert_chunk(&Arc::new(chunk3))?;
    store.insert_chunk(&Arc::new(chunk4_1))?;
    store.insert_chunk(&Arc::new(chunk4_2))?;
    store.insert_chunk(&Arc::new(chunk4_25))?;
    store.insert_chunk(&Arc::new(chunk4_3))?;
    store.insert_chunk(&Arc::new(chunk4_4))?;
    store.insert_chunk(&Arc::new(chunk5))?;

    // Each entry in `rows_at_times` corresponds to a dataframe that's expected to be returned
    // by the range query.
    // A single timepoint might have several of those! That's one of the behaviors specific to
    // range queries.
    #[allow(clippy::type_complexity)]
    let assert_range_components =
        |time_range: ResolvedTimeRange,
         component_name: ComponentName,
         row_ids_at_times: &[(TimeInt, RowId)]| {
            let timeline_frame_nr = Timeline::new("frame_nr", TimeType::Sequence);

            let query = RangeQuery::new(timeline_frame_nr, time_range);
            let results = store.range_relevant_chunks(&query, &entity_path, component_name);

            eprintln!("================= {component_name} @ {query:?} ===============");
            let mut results_processed = 0usize;
            for chunk in results {
                let chunk = chunk.range(&query, component_name);
                eprintln!("{chunk}");
                for (data_time, row_id, _array) in
                    chunk.iter_rows(&timeline_frame_nr, &component_name)
                {
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
        MyColor::name(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(ResolvedTimeRange::new(frame1, frame1), MyPoint::name(), &[]);
    assert_range_components(
        ResolvedTimeRange::new(frame2, frame2),
        MyColor::name(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame2, frame2),
        MyPoint::name(),
        &[(frame2, row_id2)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame3, frame3),
        MyColor::name(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame3, frame3),
        MyPoint::name(),
        &[(frame3, row_id3)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame4, frame4),
        MyColor::name(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame4, frame4),
        MyPoint::name(),
        &[(frame4, row_id4_25), (frame4, row_id4_4)],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame5, frame5),
        MyColor::name(),
        &[(TimeInt::STATIC, row_id5)],
    );
    assert_range_components(ResolvedTimeRange::new(frame5, frame5), MyPoint::name(), &[]);

    // Full range

    assert_range_components(
        ResolvedTimeRange::new(frame1, frame5),
        MyPoint::name(),
        &[
            (frame2, row_id2),
            (frame3, row_id3),
            (frame4, row_id4_25),
            (frame4, row_id4_4),
        ],
    );
    assert_range_components(
        ResolvedTimeRange::new(frame1, frame5),
        MyColor::name(),
        &[(TimeInt::STATIC, row_id5)],
    );

    // Infinite range

    assert_range_components(
        ResolvedTimeRange::new(TimeInt::MIN, TimeInt::MAX),
        MyPoint::name(),
        &[
            (frame2, row_id2),
            (frame3, row_id3),
            (frame4, row_id4_25),
            (frame4, row_id4_4),
        ],
    );
    assert_range_components(
        ResolvedTimeRange::new(TimeInt::MIN, TimeInt::MAX),
        MyColor::name(),
        &[(TimeInt::STATIC, row_id5)],
    );

    Ok(())
}
