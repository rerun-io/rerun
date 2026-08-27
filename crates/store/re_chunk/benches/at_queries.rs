//! Benchmarks for [`Chunk::latest_at`] and [`Chunk::earliest_at`].
//!
//! Both queries break ties on the query time with the `RowId` index. On a chunk whose row-ids are
//! sorted that costs nothing — row-ids ascend within each run of equal times, so the first valid
//! row found is already the right one. On a row-id-unsorted chunk the whole run has to be walked,
//! which is why the run length is an axis here: a chunk with one row per timestamp has runs of a
//! single row, a chunk with every row at the same timestamp is one long run.

use criterion::{Criterion, criterion_group, criterion_main};

use re_chunk::{Chunk, EarliestAtQuery, LatestAtQuery, RowId, Timeline, TimelineName};
use re_log_types::example_components::{MyPoint, MyPoints};

// ---

#[cfg(not(debug_assertions))]
const NUM_ROWS: usize = 10_000;

// `cargo test` builds the benches too: keep that a fast no-op.
#[cfg(debug_assertions)]
const NUM_ROWS: usize = 10;

const ENTITY_PATH: &str = "my/entity";

fn frame() -> TimelineName {
    TimelineName::from("frame")
}

/// How many rows share a timestamp, i.e. how long the tie-break run is.
#[derive(Clone, Copy)]
enum Times {
    /// One row per timestamp: every run is a single row.
    Unique,

    /// Every row at the same timestamp: the whole chunk is one run.
    Shared,
}

/// Whether the tie-break can be skipped, or has to walk the run.
#[derive(Clone, Copy)]
enum RowIds {
    Ascending,
    Descending,
}

fn build_chunk(times: Times, row_ids: RowIds) -> Chunk {
    let mut ids = (0..NUM_ROWS).map(|_| RowId::new()).collect::<Vec<_>>();
    if matches!(row_ids, RowIds::Descending) {
        ids.reverse();
    }

    let mut builder = Chunk::builder(ENTITY_PATH);
    for (i, row_id) in ids.into_iter().enumerate() {
        let time = match times {
            Times::Unique => i64::try_from(i).expect("row count fits in an i64"),
            Times::Shared => 0,
        };
        builder = builder.with_component_batches(
            row_id,
            [(Timeline::new_sequence("frame"), time)],
            [(
                MyPoints::descriptor_points(),
                &[MyPoint::new(1.0, 1.0)] as _,
            )],
        );
    }

    let chunk = builder.build().expect("valid chunk");

    assert_eq!(
        chunk.is_row_ids_sorted(),
        matches!(row_ids, RowIds::Ascending),
        "the fixture must exercise the intended tie-break path"
    );
    assert!(
        chunk
            .timelines()
            .get(&frame())
            .expect("fixture must carry the frame index")
            .is_sorted(),
        "the fixture must be time-sorted, to reach the sorted path"
    );

    chunk
}

fn bench_at_queries(c: &mut Criterion) {
    let num_rows = i64::try_from(NUM_ROWS).expect("row count fits in an i64");

    for (times, times_name) in [
        (Times::Unique, "unique_times"),
        (Times::Shared, "shared_time"),
    ] {
        for (row_ids, row_ids_name) in [
            (RowIds::Ascending, "row_ids_sorted"),
            (RowIds::Descending, "row_ids_unsorted"),
        ] {
            let chunk = build_chunk(times, row_ids);
            let component = MyPoints::descriptor_points().component;

            let mut group = c.benchmark_group(format!("{times_name}/{row_ids_name}"));

            // Query past the end, so the answer sits in the last run.
            let latest = LatestAtQuery::new(frame(), num_rows);
            group.bench_function("latest_at", |b| {
                b.iter(|| std::hint::black_box(chunk.latest_at(&latest, component)));
            });

            // Query before the start, so the answer sits in the first run.
            let earliest = EarliestAtQuery::new(frame(), -1);
            group.bench_function("earliest_at", |b| {
                b.iter(|| std::hint::black_box(chunk.earliest_at(&earliest, component)));
            });

            group.finish();
        }
    }
}

criterion_group!(benches, bench_at_queries);
criterion_main!(benches);
