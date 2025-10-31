// Allow unwrap() in benchmarks
#![expect(clippy::unwrap_used)]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{Criterion, criterion_group, criterion_main};
use re_chunk::{Chunk, RowId};
use re_log_encoding::Encoder;
use re_log_types::{
    entity_path, LogMsg, NonMinI64, StoreId, StoreKind, TimeInt, TimePoint, Timeline,
};
use re_types::archetypes::Points2D;
use std::sync::mpsc;

use re_data_loader::{DataLoader as _, DataLoaderSettings, RrdLoader};

#[cfg(not(debug_assertions))]
const NUM_MESSAGES: usize = 10_000;

#[cfg(debug_assertions)]
const NUM_MESSAGES: usize = 100;

criterion_group!(
    benches,
    benchmark_load_from_file_contents,
    benchmark_message_processing_batches,
);
criterion_main!(benches);

fn generate_messages(count: usize) -> Vec<LogMsg> {
    let store_id = StoreId::random(StoreKind::Recording, "bench_app");
    let mut messages = Vec::with_capacity(count);

    for i in 0..count {
        let chunk = Chunk::builder(entity_path!("points", i.to_string()))
            .with_archetype(
                RowId::new(),
                TimePoint::default().with(
                    Timeline::new_sequence("log_time"),
                    TimeInt::from_millis(NonMinI64::new(i as i64).unwrap()),
                ),
                &Points2D::new([(i as f32, i as f32), ((i + 1) as f32, (i + 1) as f32)]),
            )
            .build()
            .unwrap();

        messages.push(LogMsg::ArrowMsg(store_id.clone(), chunk.to_arrow_msg().unwrap()));
    }

    messages
}

fn encode_messages(messages: &[LogMsg]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = Encoder::new_eager(
            re_build_info::CrateVersion::LOCAL,
            re_log_encoding::rrd::EncodingOptions::PROTOBUF_UNCOMPRESSED,
            &mut bytes,
        )
        .unwrap();

        for msg in messages {
            encoder.append(msg).unwrap();
        }
        encoder.flush_blocking().unwrap();
        encoder.finish().unwrap();
    }
    bytes
}

/// Benchmark loading from file contents (parallel processing)
fn benchmark_load_from_file_contents(c: &mut Criterion) {
    let messages = generate_messages(NUM_MESSAGES);
    let encoded = encode_messages(&messages);
    let filepath = std::path::PathBuf::from("bench_data.rrd");
    let settings = DataLoaderSettings::recommended(re_log_types::RecordingId::random());

    let mut group = c.benchmark_group("load_from_file_contents");
    group.throughput(criterion::Throughput::Elements(NUM_MESSAGES as u64));

    group.bench_function("rrd_loader", |b| {
        b.iter(|| {
            let (tx, rx) = mpsc::channel();
            let contents = std::borrow::Cow::Borrowed(encoded.as_slice());
            let loader = RrdLoader;

            loader
                .load_from_file_contents(&settings, filepath.clone(), contents, tx)
                .unwrap();

            let mut count = 0;
            while rx.try_recv().is_ok() {
                count += 1;
            }

            assert_eq!(count, NUM_MESSAGES);
        });
    });

    group.finish();
}

/// Try to find optimal batch size
fn benchmark_message_processing_batches(c: &mut Criterion) {
    let messages = generate_messages(NUM_MESSAGES);
    let (tx, rx) = mpsc::channel();

    let mut group = c.benchmark_group("message_processing_batches");
    group.throughput(criterion::Throughput::Elements(NUM_MESSAGES as u64));

    // Sequential processing (baseline)
    group.bench_function("sequential", |b| {
        b.iter(|| {
            let mut batch = messages.clone();
            let loader = RrdLoader;
            for msg in batch.drain(..) {
                let data = re_data_loader::LoadedData::LogMsg(
                    loader.name(),
                    msg,
                );
                let _ = tx.send(data);
            }
            while rx.try_recv().is_ok() {}
        });
    });

    // Parallel processing with different batch sizes
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;

        for batch_size in [10, 50, 100, 500, 1000] {
            group.bench_function(format!("parallel_batch_{}", batch_size), |b| {
                b.iter(|| {
                    let mut messages = messages.clone();
                    let mut processed_count = 0;

                    while processed_count < messages.len() {
                        let remaining = messages.len() - processed_count;
                        let current_batch_size = batch_size.min(remaining);
                        let batch: Vec<_> = messages
                            .drain(processed_count..processed_count + current_batch_size)
                            .collect();

                        let processed: Vec<_> = batch
                            .into_par_iter()
                            .map(|msg| msg) // Simulate transform_message
                            .collect();

                        let loader = RrdLoader;
                        for msg in processed {
                            let data = re_data_loader::LoadedData::LogMsg(
                                loader.name(),
                                msg,
                            );
                            let _ = tx.send(data);
                        }

                        processed_count += current_batch_size;
                    }

                    while rx.try_recv().is_ok() {}
                });
            });
        }
    }

    group.finish();
}
