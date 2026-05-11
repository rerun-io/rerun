// Allow unwrap() and small numeric casts in benchmarks.
#![expect(clippy::unwrap_used)]
#![expect(clippy::cast_possible_wrap)]

//! Microbench for the per-row hot loop in `re_dataframe::QueryHandle::next_row`,
//! `next_row_batch`, and `next_n_rows`.
//!
//! Background: the redap query path
//! (`re_datafusion::dataframe_query_provider::flush`) calls `next_row` once
//! per index value and builds a fresh 1-row Arrow `RecordBatch` for each.
//! `next_n_rows` is the throughput-oriented sibling that fills per-column
//! `MutableArrayData` buffers and finalizes once per call.
//!
//! Two schemas are benched:
//! * **list-of-list-f64** — mirrors `rerun-synthetic-long-10k`
//!   (`list<list<f64>>` of length-1 outer × `VECTOR_WIDTH` inner).
//! * **list-of-struct** — mirrors `rerun-synthetic-structs-10k`
//!   (`list<struct{joint_positions: list<f64>}>`); this is the hot
//!   production schema and the primary perf target for `next_n_rows`.
//!
//! Run with:
//!
//! ```text
//! cargo bench -p re_dataframe --bench next_row
//! ```

use std::sync::Arc;

use arrow::array::{Array as _, ArrayRef, Float64Array, ListArray, StructArray};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field, Fields};
use criterion::{Criterion, criterion_group, criterion_main};

use re_chunk::{Chunk, ChunkId, EntityPath, RowId, TimePoint, Timeline};
use re_chunk_store::{ChunkStore, ChunkStoreConfig, QueryExpression};
use re_dataframe::QueryEngine;
use re_log_types::{StoreId, StoreKind, TimeInt, TimeType, TimelineName};
use re_query::QueryCache;
use re_types_core::ComponentDescriptor;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// Keep `cargo test` quick when not optimized.
#[cfg(debug_assertions)]
const ROW_COUNTS: &[usize] = &[1_000];

#[cfg(not(debug_assertions))]
const ROW_COUNTS: &[usize] = &[1_000, 10_000, 30_000];

#[cfg(not(debug_assertions))]
const NEXT_N_ROWS_BATCHES: &[usize] = &[256, 2048];

#[cfg(debug_assertions)]
const NEXT_N_ROWS_BATCHES: &[usize] = &[256];

const VECTOR_WIDTH: usize = 8;
const TIMELINE: &str = "log_time";
const ENTITY: &str = "/joints";

criterion_group!(
    benches,
    bench_next_row_list,
    bench_next_row_batch_list,
    bench_next_n_rows_list,
    bench_per_row_plus_concat_list,
    bench_next_row_struct,
    bench_next_row_batch_struct,
    bench_next_n_rows_struct,
    bench_per_row_plus_concat_struct,
    bench_next_n_rows_struct_chunks,
    bench_next_row_struct_chunks,
);
criterion_main!(benches);

#[cfg(not(debug_assertions))]
const N_CHUNKS_SWEEP: &[usize] = &[1, 8, 32, 128, 512];

#[cfg(debug_assertions)]
const N_CHUNKS_SWEEP: &[usize] = &[1, 32];

const CHUNKED_BENCH_ROWS: usize = 30_720;
const CHUNKED_BENCH_BATCH: usize = 2048;

// ---------------------------------------------------------------------------
// list-of-list-f64 (rerun-synthetic-long-10k shape)

fn build_list_of_list_f64(num_rows: usize) -> ArrayRef {
    let total_floats = num_rows * VECTOR_WIDTH;
    let values = Float64Array::from_iter_values((0..total_floats).map(|i| (i as f64) * 0.001));

    let inner_offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(VECTOR_WIDTH, num_rows));
    let inner_field = Arc::new(Field::new("item", DataType::Float64, false));
    let inner_list = ListArray::new(inner_field.clone(), inner_offsets, Arc::new(values), None);

    let outer_offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(1usize, num_rows));
    let outer_field = Arc::new(Field::new("item", DataType::List(inner_field), false));
    let outer_list = ListArray::new(outer_field, outer_offsets, Arc::new(inner_list), None);

    Arc::new(outer_list)
}

// ---------------------------------------------------------------------------
// list-of-struct (rerun-synthetic-structs-10k shape)

fn build_list_of_struct(num_rows: usize) -> ArrayRef {
    let total_floats = num_rows * VECTOR_WIDTH;
    let values = Float64Array::from_iter_values((0..total_floats).map(|i| (i as f64) * 0.001));

    // joint_positions: list<f64> with length VECTOR_WIDTH per row
    let positions_offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(VECTOR_WIDTH, num_rows));
    let positions_inner_field = Arc::new(Field::new("item", DataType::Float64, false));
    let positions_list = ListArray::new(
        positions_inner_field,
        positions_offsets,
        Arc::new(values),
        None,
    );

    // struct { joint_positions: list<f64> }
    let positions_field = Arc::new(Field::new(
        "joint_positions",
        positions_list.data_type().clone(),
        false,
    ));
    let struct_fields = Fields::from(vec![positions_field.clone()]);
    let struct_array = StructArray::new(
        struct_fields.clone(),
        vec![Arc::new(positions_list) as ArrayRef],
        None,
    );

    // outer list of length 1 per row
    let outer_offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(1usize, num_rows));
    let outer_field = Arc::new(Field::new("item", DataType::Struct(struct_fields), false));
    let outer_list = ListArray::new(outer_field, outer_offsets, Arc::new(struct_array), None);

    Arc::new(outer_list)
}

// ---------------------------------------------------------------------------

fn build_chunk(num_rows: usize, build_outer: fn(usize) -> ArrayRef) -> Arc<Chunk> {
    let timeline = Timeline::new(TIMELINE, TimeType::TimestampNs);
    let component_descr = ComponentDescriptor {
        archetype: Some("schemas.proto.JointState".into()),
        component: "schemas.proto.JointState:joint_positions".into(),
        component_type: None,
    };

    let mut builder = Chunk::builder_with_id(ChunkId::new(), EntityPath::from(ENTITY));

    let outer = build_outer(num_rows);
    for row_idx in 0..num_rows {
        let one_row = outer.slice(row_idx, 1);
        let mut tp = TimePoint::default();
        tp.insert(timeline, TimeInt::new_temporal(row_idx as i64));
        builder = builder.with_row(RowId::new(), tp, [(component_descr.clone(), one_row)]);
    }

    Arc::new(builder.build().unwrap())
}

fn make_engine(
    num_rows: usize,
    build_outer: fn(usize) -> ArrayRef,
) -> QueryEngine<re_query::StorageEngine> {
    let store = ChunkStore::new_handle(
        StoreId::random(StoreKind::Recording, "bench_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );
    {
        let mut s = store.write();
        s.insert_chunk(&build_chunk(num_rows, build_outer)).unwrap();
    }
    let cache = QueryCache::new_handle(store.clone());
    QueryEngine::new(store, cache)
}

fn bench_query() -> QueryExpression {
    QueryExpression {
        filtered_index: Some(TimelineName::new(TIMELINE)),
        ..Default::default()
    }
}

/// Drives the engine through the same shape as `send_next_row_batch`:
/// accumulate up to `batch_size` rows from `next_row()` then `concat_arrays`
/// per column. Returns total rows.
fn run_per_row_plus_concat(
    engine: &QueryEngine<re_query::StorageEngine>,
    batch_size: usize,
) -> usize {
    let mut handle = engine.query(bench_query());
    let num_fields = handle.schema().fields.len();
    let mut total_rows = 0usize;

    'outer: loop {
        let mut row_groups: Vec<Vec<ArrayRef>> = Vec::with_capacity(batch_size);
        let mut acc_rows = 0usize;
        while acc_rows < batch_size {
            let Some(row) = handle.next_row() else {
                if row_groups.is_empty() {
                    break 'outer;
                }
                break;
            };
            acc_rows += row[0].len();
            row_groups.push(row);
        }
        if row_groups.is_empty() {
            break;
        }
        // Per-column concat (mirrors `send_next_row_batch`).
        for col_idx in 0..num_fields {
            let parts: Vec<&dyn arrow::array::Array> =
                row_groups.iter().map(|r| r[col_idx].as_ref()).collect();
            let _combined = re_arrow_util::concat_arrays(&parts).unwrap();
        }
        total_rows += acc_rows;
    }

    total_rows
}

// ---------------------------------------------------------------------------
// list-of-list benches

fn bench_next_row_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("next_row_list");
    for &n in ROW_COUNTS {
        let engine = make_engine(n, build_list_of_list_f64);
        group.throughput(criterion::Throughput::Elements(n as u64));
        group.bench_function(format!("rows={n}"), |b| {
            b.iter(|| {
                let mut handle = engine.query(bench_query());
                let mut count = 0usize;
                while handle.next_row().is_some() {
                    count += 1;
                }
                criterion::black_box(count)
            });
        });
    }
}

fn bench_next_row_batch_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("next_row_batch_list");
    for &n in ROW_COUNTS {
        let engine = make_engine(n, build_list_of_list_f64);
        group.throughput(criterion::Throughput::Elements(n as u64));
        group.bench_function(format!("rows={n}"), |b| {
            b.iter(|| {
                let mut handle = engine.query(bench_query());
                let mut total_rows = 0usize;
                while let Some(rb) = handle.next_row_batch() {
                    total_rows += rb.num_rows();
                }
                criterion::black_box(total_rows)
            });
        });
    }
}

/// Mimics `send_next_row_batch`: accumulate N `next_row()` results, then concat per column.
/// Single-shot finalization comparable to a single `next_n_rows(N)` call.
fn bench_per_row_plus_concat_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_row_plus_concat_list");
    for &n in ROW_COUNTS {
        let engine = make_engine(n, build_list_of_list_f64);
        for &batch_size in NEXT_N_ROWS_BATCHES {
            group.throughput(criterion::Throughput::Elements(n as u64));
            group.bench_function(format!("rows={n} batch={batch_size}"), |b| {
                b.iter(|| run_per_row_plus_concat(&engine, batch_size));
            });
        }
    }
}

fn bench_next_n_rows_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("next_n_rows_list");
    for &n in ROW_COUNTS {
        let engine = make_engine(n, build_list_of_list_f64);
        for &batch_size in NEXT_N_ROWS_BATCHES {
            group.throughput(criterion::Throughput::Elements(n as u64));
            group.bench_function(format!("rows={n} batch={batch_size}"), |b| {
                b.iter(|| {
                    let mut handle = engine.query(bench_query());
                    let mut total_rows = 0usize;
                    loop {
                        let out = handle.next_n_rows(batch_size, usize::MAX);
                        if out.num_rows == 0 {
                            break;
                        }
                        total_rows += out.num_rows;
                    }
                    criterion::black_box(total_rows)
                });
            });
        }
    }
}

// ---------------------------------------------------------------------------
// list-of-struct benches

fn bench_next_row_struct(c: &mut Criterion) {
    let mut group = c.benchmark_group("next_row_struct");
    for &n in ROW_COUNTS {
        let engine = make_engine(n, build_list_of_struct);
        group.throughput(criterion::Throughput::Elements(n as u64));
        group.bench_function(format!("rows={n}"), |b| {
            b.iter(|| {
                let mut handle = engine.query(bench_query());
                let mut count = 0usize;
                while handle.next_row().is_some() {
                    count += 1;
                }
                criterion::black_box(count)
            });
        });
    }
}

fn bench_next_row_batch_struct(c: &mut Criterion) {
    let mut group = c.benchmark_group("next_row_batch_struct");
    for &n in ROW_COUNTS {
        let engine = make_engine(n, build_list_of_struct);
        group.throughput(criterion::Throughput::Elements(n as u64));
        group.bench_function(format!("rows={n}"), |b| {
            b.iter(|| {
                let mut handle = engine.query(bench_query());
                let mut total_rows = 0usize;
                while let Some(rb) = handle.next_row_batch() {
                    total_rows += rb.num_rows();
                }
                criterion::black_box(total_rows)
            });
        });
    }
}

fn bench_per_row_plus_concat_struct(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_row_plus_concat_struct");
    for &n in ROW_COUNTS {
        let engine = make_engine(n, build_list_of_struct);
        for &batch_size in NEXT_N_ROWS_BATCHES {
            group.throughput(criterion::Throughput::Elements(n as u64));
            group.bench_function(format!("rows={n} batch={batch_size}"), |b| {
                b.iter(|| run_per_row_plus_concat(&engine, batch_size));
            });
        }
    }
}

/// Build a `QueryEngine` whose view contains `n_chunks` non-overlapping chunks
/// of `total_rows / n_chunks` time-monotonic rows each. Returns the engine
/// alongside the actual row count inserted (`rows_per_chunk * n_chunks`),
/// which may be less than `total_rows` due to integer truncation. Callers
/// should use the returned count for throughput labels.
fn build_chunked_engine(
    total_rows: usize,
    n_chunks: usize,
    build_outer: fn(usize) -> ArrayRef,
) -> (QueryEngine<re_query::StorageEngine>, usize) {
    assert!(n_chunks >= 1);
    // Disable compaction so the bench actually measures `n_chunks` chunks.
    // With small contiguous chunks the default config will merge them on
    // insert and collapse the chunk-count sweep.
    let store = ChunkStore::new_handle(
        StoreId::random(StoreKind::Recording, "bench_app"),
        ChunkStoreConfig::COMPACTION_DISABLED,
    );
    let timeline = Timeline::new(TIMELINE, TimeType::TimestampNs);
    let component_descr = ComponentDescriptor {
        archetype: Some("schemas.proto.JointState".into()),
        component: "schemas.proto.JointState:joint_positions".into(),
        component_type: None,
    };

    let rows_per_chunk = total_rows / n_chunks;
    {
        let mut s = store.write();
        for chunk_idx in 0..n_chunks {
            let outer = build_outer(rows_per_chunk);
            let mut builder = Chunk::builder_with_id(ChunkId::new(), EntityPath::from(ENTITY));
            for local_row in 0..rows_per_chunk {
                let global_row = chunk_idx * rows_per_chunk + local_row;
                let one_row = outer.slice(local_row, 1);
                let mut tp = TimePoint::default();
                tp.insert(timeline, TimeInt::new_temporal(global_row as i64));
                builder = builder.with_row(RowId::new(), tp, [(component_descr.clone(), one_row)]);
            }
            let chunk = Arc::new(builder.build().unwrap());
            s.insert_chunk(&chunk).unwrap();
        }
    }
    let cache = QueryCache::new_handle(store.clone());
    let actual_rows = (total_rows / n_chunks) * n_chunks;
    (QueryEngine::new(store, cache), actual_rows)
}

fn bench_next_n_rows_struct_chunks(c: &mut Criterion) {
    let mut group = c.benchmark_group("next_n_rows_struct_chunks");
    for &n_chunks in N_CHUNKS_SWEEP {
        let (engine, actual_rows) =
            build_chunked_engine(CHUNKED_BENCH_ROWS, n_chunks, build_list_of_struct);
        group.throughput(criterion::Throughput::Elements(actual_rows as u64));
        group.bench_function(format!("chunks={n_chunks}"), |b| {
            b.iter(|| {
                let mut handle = engine.query(bench_query());
                let mut total_rows = 0usize;
                loop {
                    let out = handle.next_n_rows(CHUNKED_BENCH_BATCH, usize::MAX);
                    if out.num_rows == 0 {
                        break;
                    }
                    total_rows += out.num_rows;
                }
                criterion::black_box(total_rows)
            });
        });
    }
}

fn bench_next_row_struct_chunks(c: &mut Criterion) {
    let mut group = c.benchmark_group("next_row_struct_chunks");
    for &n_chunks in N_CHUNKS_SWEEP {
        let (engine, actual_rows) =
            build_chunked_engine(CHUNKED_BENCH_ROWS, n_chunks, build_list_of_struct);
        group.throughput(criterion::Throughput::Elements(actual_rows as u64));
        group.bench_function(format!("chunks={n_chunks}"), |b| {
            b.iter(|| {
                let mut handle = engine.query(bench_query());
                let mut count = 0usize;
                while handle.next_row().is_some() {
                    count += 1;
                }
                criterion::black_box(count)
            });
        });
    }
}

fn bench_next_n_rows_struct(c: &mut Criterion) {
    let mut group = c.benchmark_group("next_n_rows_struct");
    for &n in ROW_COUNTS {
        let engine = make_engine(n, build_list_of_struct);
        for &batch_size in NEXT_N_ROWS_BATCHES {
            group.throughput(criterion::Throughput::Elements(n as u64));
            group.bench_function(format!("rows={n} batch={batch_size}"), |b| {
                b.iter(|| {
                    let mut handle = engine.query(bench_query());
                    let mut total_rows = 0usize;
                    loop {
                        let out = handle.next_n_rows(batch_size, usize::MAX);
                        if out.num_rows == 0 {
                            break;
                        }
                        total_rows += out.num_rows;
                    }
                    criterion::black_box(total_rows)
                });
            });
        }
    }
}
