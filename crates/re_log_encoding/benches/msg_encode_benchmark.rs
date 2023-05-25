#[cfg(not(all(feature = "decoder", feature = "encoder")))]
compile_error!("msg_encode_benchmark requires 'decoder' and 'encoder' features.");

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use re_log_types::{
    datagen::{build_frame_nr, build_some_colors, build_some_point2d},
    entity_path, DataRow, DataTable, Index, LogMsg, RecordingId, RecordingType, RowId, TableId,
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
    re_log_encoding::encoder::encode(encoding_options, messages.iter(), &mut bytes).unwrap();
    assert!(bytes.len() > messages.len());
    bytes
}

fn decode_log_msgs(mut bytes: &[u8]) -> Vec<LogMsg> {
    let messages = re_log_encoding::decoder::Decoder::new(&mut bytes)
        .unwrap()
        .collect::<Result<Vec<LogMsg>, _>>()
        .unwrap();
    assert!(bytes.is_empty());
    messages
}

fn generate_messages(recording_id: &RecordingId, tables: &[DataTable]) -> Vec<LogMsg> {
    tables
        .iter()
        .map(|table| LogMsg::ArrowMsg(recording_id.clone(), table.to_arrow_msg().unwrap()))
        .collect()
}

fn decode_tables(messages: &[LogMsg]) -> Vec<DataTable> {
    messages
        .iter()
        .map(|log_msg| {
            if let LogMsg::ArrowMsg(_, arrow_msg) = log_msg {
                DataTable::from_arrow_msg(arrow_msg).unwrap()
            } else {
                unreachable!()
            }
        })
        .collect()
}

fn mono_points_arrow(c: &mut Criterion) {
    fn generate_tables() -> Vec<DataTable> {
        (0..NUM_POINTS)
            .map(|i| {
                DataTable::from_rows(
                    TableId::ZERO,
                    [DataRow::from_cells2(
                        RowId::ZERO,
                        entity_path!("points", Index::Sequence(i as _)),
                        [build_frame_nr(0.into())],
                        1,
                        (build_some_point2d(1), build_some_colors(1)),
                    )],
                )
            })
            .collect()
    }

    {
        let recording_id = RecordingId::random(RecordingType::Data);
        let mut group = c.benchmark_group("mono_points_arrow");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_message_bundles", |b| {
            b.iter(generate_tables);
        });
        let tables = generate_tables();
        group.bench_function("generate_messages", |b| {
            b.iter(|| generate_messages(&recording_id, &tables));
        });
        let messages = generate_messages(&recording_id, &tables);
        group.bench_function("encode_log_msg", |b| {
            b.iter(|| encode_log_msgs(&messages));
        });
        group.bench_function("encode_total", |b| {
            b.iter(|| encode_log_msgs(&generate_messages(&recording_id, &generate_tables())));
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
                let tables = decode_tables(&messages);
                assert_eq!(tables.len(), messages.len());
                tables
            });
        });
        group.bench_function("decode_total", |b| {
            b.iter(|| decode_tables(&decode_log_msgs(&encoded)));
        });
    }
}

fn mono_points_arrow_batched(c: &mut Criterion) {
    fn generate_table() -> DataTable {
        DataTable::from_rows(
            TableId::ZERO,
            (0..NUM_POINTS).map(|i| {
                DataRow::from_cells2(
                    RowId::ZERO,
                    entity_path!("points", Index::Sequence(i as _)),
                    [build_frame_nr(0.into())],
                    1,
                    (build_some_point2d(1), build_some_colors(1)),
                )
            }),
        )
    }

    {
        let recording_id = RecordingId::random(RecordingType::Data);
        let mut group = c.benchmark_group("mono_points_arrow_batched");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_message_bundles", |b| {
            b.iter(generate_table);
        });
        let tables = [generate_table()];
        group.bench_function("generate_messages", |b| {
            b.iter(|| generate_messages(&recording_id, &tables));
        });
        let messages = generate_messages(&recording_id, &tables);
        group.bench_function("encode_log_msg", |b| {
            b.iter(|| encode_log_msgs(&messages));
        });
        group.bench_function("encode_total", |b| {
            b.iter(|| encode_log_msgs(&generate_messages(&recording_id, &[generate_table()])));
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
                let bundles = decode_tables(&messages);
                assert_eq!(bundles.len(), messages.len());
                bundles
            });
        });
        group.bench_function("decode_total", |b| {
            b.iter(|| decode_tables(&decode_log_msgs(&encoded)));
        });
    }
}

fn batch_points_arrow(c: &mut Criterion) {
    fn generate_tables() -> Vec<DataTable> {
        vec![DataTable::from_rows(
            TableId::ZERO,
            [DataRow::from_cells2(
                RowId::ZERO,
                entity_path!("points"),
                [build_frame_nr(0.into())],
                NUM_POINTS as _,
                (
                    build_some_point2d(NUM_POINTS),
                    build_some_colors(NUM_POINTS),
                ),
            )],
        )]
    }

    {
        let recording_id = RecordingId::random(RecordingType::Data);
        let mut group = c.benchmark_group("batch_points_arrow");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_message_bundles", |b| {
            b.iter(generate_tables);
        });
        let tables = generate_tables();
        group.bench_function("generate_messages", |b| {
            b.iter(|| generate_messages(&recording_id, &tables));
        });
        let messages = generate_messages(&recording_id, &tables);
        group.bench_function("encode_log_msg", |b| {
            b.iter(|| encode_log_msgs(&messages));
        });
        group.bench_function("encode_total", |b| {
            b.iter(|| encode_log_msgs(&generate_messages(&recording_id, &generate_tables())));
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
                let tables = decode_tables(&messages);
                assert_eq!(tables.len(), messages.len());
                tables
            });
        });
        group.bench_function("decode_total", |b| {
            b.iter(|| decode_tables(&decode_log_msgs(&encoded)));
        });
    }
}
