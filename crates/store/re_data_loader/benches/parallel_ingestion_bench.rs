// Allow unwrap() in benchmarks
#![expect(clippy::unwrap_used)]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{Criterion, criterion_group, criterion_main};
use re_chunk::{Chunk, RowId};
use re_log_encoding::Encoder;
use re_log_types::{
    LogMsg, NonMinI64, StoreId, StoreKind, TimeInt, TimePoint, Timeline, entity_path,
};
use re_types::archetypes::Points2D;
use std::sync::mpsc;

use re_data_loader::{DataLoader as _, DataLoaderSettings, RrdLoader};

#[cfg(not(debug_assertions))]
const NUM_MESSAGES: usize = 10_000;

#[cfg(debug_assertions)]
const NUM_MESSAGES: usize = 100;

criterion_group!(benches, benchmark_load_from_file_contents);
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
                    TimeInt::from_millis(NonMinI64::new(i64::try_from(i).unwrap()).unwrap()),
                ),
                &Points2D::new([(i as f32, i as f32), ((i + 1) as f32, (i + 1) as f32)]),
            )
            .build()
            .unwrap();

        messages.push(LogMsg::ArrowMsg(
            store_id.clone(),
            chunk.to_arrow_msg().unwrap(),
        ));
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

/// Benchmark loading from file (parallel processing)
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
