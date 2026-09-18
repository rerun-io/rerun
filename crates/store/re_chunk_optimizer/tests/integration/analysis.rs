//! Tests for the chunk index analysis, on synthetic chunk indexes, through the public API.
//!
//! Chunks are built with deterministic IDs, turned into a real `RawRrdManifest` through
//! `RawRrdManifest::build_in_memory_from_chunks`, and analyzed from there — the same path the
//! production code takes, minus the file.

use re_chunk::{Chunk, ChunkId, RowId};
use re_chunk_optimizer::{ChunkIndexAnalysis, analyze_chunk_index};
use re_log_encoding::RawRrdManifest;
use re_log_types::example_components::{MyColor, MyPoints};
use re_log_types::{StoreId, StoreKind, TimePoint, Timeline};
use re_types_core::ComponentBatch as _;

/// The byte target the catalog server story is built around; owned by the profile.
const OBJECT_STORE_CHUNK_MAX_BYTES: u64 =
    re_chunk_store::OptimizationProfile::OBJECT_STORE.chunk_max_bytes;

fn frame() -> Timeline {
    Timeline::new_sequence("frame")
}

fn sensor_time() -> Timeline {
    Timeline::new_duration("sensor_time")
}

/// One chunk with one row per entry of `rows`; each row carries the given `(timeline, time)`s.
///
/// An empty `rows` slice builds a static chunk with one row.
fn chunk(id_seed: u128, entity_path: &str, rows: &[Vec<(Timeline, i64)>]) -> anyhow::Result<Chunk> {
    let mut builder = Chunk::builder_with_id(ChunkId::from_u128(id_seed), entity_path);

    let colors = |i: u32| MyColor::from_iter(i..=i);

    if rows.is_empty() {
        builder = builder.with_serialized_batches(
            RowId::from_u128(id_seed << 32),
            TimePoint::default(),
            [colors(0).try_serialized(MyPoints::descriptor_colors())?],
        );
    }

    for (i, row) in rows.iter().enumerate() {
        let timepoint = row
            .iter()
            .fold(TimePoint::default(), |timepoint, (timeline, time)| {
                timepoint.with(*timeline, *time)
            });
        builder = builder.with_serialized_batches(
            RowId::from_u128((id_seed << 32) + i as u128 + 1),
            timepoint,
            [colors(i as u32).try_serialized(MyPoints::descriptor_colors())?],
        );
    }

    Ok(builder.build()?)
}

fn chunk_index(chunks: &[Chunk]) -> anyhow::Result<RawRrdManifest> {
    let store_id = StoreId::new(StoreKind::Recording, "test_app", "test_recording");
    Ok(RawRrdManifest::build_in_memory_from_chunks(
        store_id,
        chunks.iter(),
    )?)
}

fn analyze(chunks: &[Chunk], chunk_max_bytes: u64) -> anyhow::Result<ChunkIndexAnalysis> {
    Ok(analyze_chunk_index(&chunk_index(chunks)?, chunk_max_bytes)?)
}

#[test]
fn mixed_timeline_sets_partition_the_entity() -> anyhow::Result<()> {
    // The two chunks overlap on `frame`, but they carry different timeline sets, so they sit in
    // different groups holding a single chunk each: nothing overlaps within a group, and merging
    // across groups is impossible anyway.
    let chunks = vec![
        chunk(1, "entity", &[vec![(frame(), 0)], vec![(frame(), 10)]])?,
        chunk(
            2,
            "entity",
            &[
                vec![(frame(), 5), (sensor_time(), 0)],
                vec![(frame(), 15), (sensor_time(), 10)],
            ],
        )?,
    ];
    let analysis = analyze(&chunks, OBJECT_STORE_CHUNK_MAX_BYTES)?;

    assert_eq!(analysis.merge.actual_chunks, 2);
    assert_eq!(analysis.merge.achievable_chunks, 2); // the groups cannot merge with each other

    Ok(())
}

#[test]
fn merge_assessment_counts_excess_chunks() -> anyhow::Result<()> {
    // Eight tiny chunks on one entity: they would all merge into one.
    let chunks: Vec<Chunk> = (0..8)
        .map(|i| {
            chunk(
                i as u128 + 1,
                "entity",
                &[vec![(frame(), i * 10)], vec![(frame(), i * 10 + 5)]],
            )
        })
        .collect::<anyhow::Result<_>>()?;
    let assessment = analyze(&chunks, OBJECT_STORE_CHUNK_MAX_BYTES)?.merge;

    assert_eq!(assessment.actual_chunks, 8);
    assert_eq!(assessment.achievable_chunks, 1);
    assert_eq!(assessment.excess_chunks, 7);
    assert_eq!(assessment.factor, 8.0);

    assert!(assessment.looks_unoptimized_with(4.0, 5));
    assert!(!assessment.looks_unoptimized_with(4.0, 10_000)); // absolute gate holds it back
    assert!(!assessment.looks_unoptimized_with(10.0, 5)); // factor gate holds it back
    assert!(!assessment.looks_unoptimized()); // the real thresholds require 200 excess chunks

    Ok(())
}

#[test]
fn merge_assessment_respects_entity_floors() -> anyhow::Result<()> {
    // One tiny chunk per entity: nothing can merge, whatever the byte target says.
    let chunks = vec![
        chunk(1, "a", &[vec![(frame(), 0)]])?,
        chunk(2, "b", &[vec![(frame(), 0)]])?,
        chunk(3, "c", &[vec![(frame(), 0)]])?,
    ];
    let assessment = analyze(&chunks, OBJECT_STORE_CHUNK_MAX_BYTES)?.merge;

    assert_eq!(assessment.actual_chunks, 3);
    assert_eq!(assessment.achievable_chunks, 3);
    assert_eq!(assessment.excess_chunks, 0);
    assert_eq!(assessment.factor, 1.0);

    Ok(())
}

#[test]
fn merge_assessment_clamps_oversized_chunks() -> anyhow::Result<()> {
    // With a 1-byte target every chunk is oversized; achievable clamps to the actual count
    // instead of pretending the chunks should be split.
    let chunks = vec![
        chunk(1, "entity", &[vec![(frame(), 0)]])?,
        chunk(2, "entity", &[vec![(frame(), 10)]])?,
    ];
    let assessment = analyze(&chunks, 1)?.merge;

    assert_eq!(assessment.actual_chunks, 2);
    assert_eq!(assessment.achievable_chunks, 2);
    assert_eq!(assessment.factor, 1.0);

    Ok(())
}

#[test]
fn snapshots() -> anyhow::Result<()> {
    let log_time = Timeline::log_time();
    let chunks = vec![
        chunk(1, "static_entity", &[])?,
        chunk(
            2,
            "adjacent",
            &[
                vec![(frame(), 0), (log_time, 0)],
                vec![(frame(), 10), (log_time, 1_000_000_000)],
            ],
        )?,
        chunk(
            3,
            "adjacent",
            &[
                vec![(frame(), 11), (log_time, 2_000_000_000)],
                vec![(frame(), 20), (log_time, 3_000_000_000)],
            ],
        )?,
        chunk(4, "overlapping", &[vec![(frame(), 0)], vec![(frame(), 10)]])?,
        chunk(5, "overlapping", &[vec![(frame(), 5)], vec![(frame(), 20)]])?,
    ];
    let analysis = analyze(&chunks, OBJECT_STORE_CHUNK_MAX_BYTES)?;

    insta::assert_debug_snapshot!("merge_assessment", analysis.merge);
    assert!(analysis.num_columns > 0);

    Ok(())
}
