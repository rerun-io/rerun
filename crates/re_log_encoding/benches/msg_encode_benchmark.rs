// Allow unwrap() in benchmarks
#![allow(clippy::unwrap_used)]

#[cfg(not(all(feature = "decoder", feature = "encoder")))]
compile_error!("msg_encode_benchmark requires 'decoder' and 'encoder' features.");

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use re_chunk::{Chunk, RowId, TransportChunk};
use re_log_types::{
    entity_path,
    example_components::{MyColor, MyPoint},
    LogMsg, StoreId, StoreKind, TimeInt, TimeType, Timeline,
};

use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(not(debug_assertions))]
const NUM_POINTS: usize = 10_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_POINTS: usize = 1;

criterion_group!(
    benches,
    mono_points_arrow,
    mono_points_arrow_batched,
    batch_points_arrow,
);
criterion_main!(benches);

fn encode_log_msgs(messages: &[LogMsg]) -> Vec<u8> {
    let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
    let mut bytes = vec![];
    re_log_encoding::encoder::encode(
        re_build_info::CrateVersion::LOCAL,
        encoding_options,
        messages.iter(),
        &mut bytes,
    )
    .unwrap();
    assert!(bytes.len() > messages.len());
    bytes
}

fn decode_log_msgs(mut bytes: &[u8]) -> Vec<LogMsg> {
    let version_policy = re_log_encoding::decoder::VersionPolicy::Error;
    let messages = re_log_encoding::decoder::Decoder::new(version_policy, &mut bytes)
        .unwrap()
        .collect::<Result<Vec<LogMsg>, _>>()
        .unwrap();
    assert!(bytes.is_empty());
    messages
}

fn generate_messages(store_id: &StoreId, chunks: &[Chunk]) -> Vec<LogMsg> {
    chunks
        .iter()
        .map(|chunk| LogMsg::ArrowMsg(store_id.clone(), chunk.to_arrow_msg().unwrap()))
        .collect()
}

fn decode_chunks(messages: &[LogMsg]) -> Vec<Chunk> {
    messages
        .iter()
        .map(|log_msg| {
            if let LogMsg::ArrowMsg(_, arrow_msg) = log_msg {
                Chunk::from_transport(&TransportChunk {
                    schema: arrow_msg.schema.clone(),
                    data: arrow_msg.chunk.clone(),
                })
                .unwrap()
            } else {
                unreachable!()
            }
        })
        .collect()
}

fn mono_points_arrow(c: &mut Criterion) {
    fn generate_chunks() -> Vec<Chunk> {
        (0..NUM_POINTS)
            .map(|i| {
                Chunk::builder(entity_path!("points", i.to_string()))
                    .with_component_batches(
                        RowId::ZERO,
                        [build_frame_nr(TimeInt::ZERO)],
                        [
                            &MyPoint::from_iter(0..1) as _,
                            &MyColor::from_iter(0..1) as _,
                        ],
                    )
                    .build()
                    .unwrap()
            })
            .collect()
    }

    {
        let store_id = StoreId::random(StoreKind::Recording);
        let mut group = c.benchmark_group("mono_points_arrow");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_message_bundles", |b| {
            b.iter(generate_chunks);
        });
        let chunks = generate_chunks();
        group.bench_function("generate_messages", |b| {
            b.iter(|| generate_messages(&store_id, &chunks));
        });
        let messages = generate_messages(&store_id, &chunks);
        group.bench_function("encode_log_msg", |b| {
            b.iter(|| encode_log_msgs(&messages));
        });
        group.bench_function("encode_total", |b| {
            b.iter(|| encode_log_msgs(&generate_messages(&store_id, &generate_chunks())));
        });

        let encoded = encode_log_msgs(&messages);
        group.bench_function("decode_log_msg", |b| {
            b.iter(|| {
                let decoded = decode_log_msgs(&encoded);
                assert_eq!(decoded.len(), messages.len());
                decoded
            });
        });
        group.bench_function("decode_message_bundles", |b| {
            b.iter(|| {
                let chunks = decode_chunks(&messages);
                assert_eq!(chunks.len(), messages.len());
                chunks
            });
        });
        group.bench_function("decode_total", |b| {
            b.iter(|| decode_chunks(&decode_log_msgs(&encoded)));
        });
    }
}

fn mono_points_arrow_batched(c: &mut Criterion) {
    fn generate_chunk() -> Chunk {
        let mut builder = Chunk::builder("points".into());
        for _ in 0..NUM_POINTS {
            builder = builder.with_component_batches(
                RowId::ZERO,
                [build_frame_nr(TimeInt::ZERO)],
                [
                    &MyPoint::from_iter(0..1) as _,
                    &MyColor::from_iter(0..1) as _,
                ],
            );
        }
        builder.build().unwrap()
    }

    {
        let store_id = StoreId::random(StoreKind::Recording);
        let mut group = c.benchmark_group("mono_points_arrow_batched");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_message_bundles", |b| {
            b.iter(generate_chunk);
        });
        let chunks = [generate_chunk()];
        group.bench_function("generate_messages", |b| {
            b.iter(|| generate_messages(&store_id, &chunks));
        });
        let messages = generate_messages(&store_id, &chunks);
        group.bench_function("encode_log_msg", |b| {
            b.iter(|| encode_log_msgs(&messages));
        });
        group.bench_function("encode_total", |b| {
            b.iter(|| encode_log_msgs(&generate_messages(&store_id, &[generate_chunk()])));
        });

        let encoded = encode_log_msgs(&messages);
        group.bench_function("decode_log_msg", |b| {
            b.iter(|| {
                let decoded = decode_log_msgs(&encoded);
                assert_eq!(decoded.len(), messages.len());
                decoded
            });
        });
        group.bench_function("decode_message_bundles", |b| {
            b.iter(|| {
                let bundles = decode_chunks(&messages);
                assert_eq!(bundles.len(), messages.len());
                bundles
            });
        });
        group.bench_function("decode_total", |b| {
            b.iter(|| decode_chunks(&decode_log_msgs(&encoded)));
        });
    }
}

fn batch_points_arrow(c: &mut Criterion) {
    fn generate_chunks() -> Vec<Chunk> {
        vec![Chunk::builder(entity_path!("points"))
            .with_component_batches(
                RowId::ZERO,
                [build_frame_nr(TimeInt::ZERO)],
                [
                    &MyPoint::from_iter(0..NUM_POINTS as u32) as _,
                    &MyColor::from_iter(0..NUM_POINTS as u32) as _,
                ],
            )
            .build()
            .unwrap()]
    }

    {
        let store_id = StoreId::random(StoreKind::Recording);
        let mut group = c.benchmark_group("batch_points_arrow");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_message_bundles", |b| {
            b.iter(generate_chunks);
        });
        let chunks = generate_chunks();
        group.bench_function("generate_messages", |b| {
            b.iter(|| generate_messages(&store_id, &chunks));
        });
        let messages = generate_messages(&store_id, &chunks);
        group.bench_function("encode_log_msg", |b| {
            b.iter(|| encode_log_msgs(&messages));
        });
        group.bench_function("encode_total", |b| {
            b.iter(|| encode_log_msgs(&generate_messages(&store_id, &generate_chunks())));
        });

        let encoded = encode_log_msgs(&messages);
        group.bench_function("decode_log_msg", |b| {
            b.iter(|| {
                let decoded = decode_log_msgs(&encoded);
                assert_eq!(decoded.len(), messages.len());
                decoded
            });
        });
        group.bench_function("decode_message_bundles", |b| {
            b.iter(|| {
                let chunks = decode_chunks(&messages);
                assert_eq!(chunks.len(), messages.len());
                chunks
            });
        });
        group.bench_function("decode_total", |b| {
            b.iter(|| decode_chunks(&decode_log_msgs(&encoded)));
        });
    }
}

/// Build a ([`Timeline`], [`TimeInt`]) tuple from `frame_nr` suitable for inserting in a [`re_log_types::TimePoint`].
fn build_frame_nr(frame_nr: TimeInt) -> (Timeline, TimeInt) {
    (Timeline::new("frame_nr", TimeType::Sequence), frame_nr)
}
