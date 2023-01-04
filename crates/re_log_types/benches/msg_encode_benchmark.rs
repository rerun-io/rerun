#[cfg(not(all(feature = "save", feature = "load")))]
compile_error!("msg_encode_benchmark requires 'save' and 'load' features.");

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use re_log_types::{
    datagen::{build_frame_nr, build_some_colors, build_some_point2d},
    msg_bundle::{try_build_msg_bundle2, MsgBundle},
    obj_path, ArrowMsg, BatchIndex, Data, DataMsg, DataPath, DataVec, FieldName, Index, LogMsg,
    LoggedData, MsgId, TimeInt, TimePoint, Timeline,
};

use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(not(debug_assertions))]
const NUM_POINTS: usize = 10_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_POINTS: usize = 1;

criterion_group!(
    benches,
    mono_points_classic,
    mono_points_arrow,
    batch_points_classic,
    batch_points_arrow,
);
criterion_main!(benches);

const COLOR: [u8; 4] = [255, 255, 255, 255];
const POS: [f32; 2] = [2.0, 3.0];

fn encode_log_msgs(messages: &[LogMsg]) -> Vec<u8> {
    let mut bytes = vec![];
    re_log_types::encoding::encode(messages.iter(), &mut bytes).unwrap();
    assert!(bytes.len() > messages.len());
    bytes
}

fn decode_log_msgs(mut bytes: &[u8]) -> Vec<LogMsg> {
    let messages = re_log_types::encoding::Decoder::new(&mut bytes)
        .unwrap()
        .collect::<anyhow::Result<Vec<LogMsg>>>()
        .unwrap();
    assert!(bytes.is_empty());
    messages
}

fn generate_messages(bundles: &[MsgBundle]) -> Vec<LogMsg> {
    bundles
        .iter()
        .map(|bundle| LogMsg::ArrowMsg(ArrowMsg::try_from(bundle.clone()).unwrap()))
        .collect()
}

fn decode_message_bundles(messages: &[LogMsg]) -> Vec<MsgBundle> {
    messages
        .iter()
        .map(|log_msg| {
            if let LogMsg::ArrowMsg(arrow_msg) = log_msg {
                MsgBundle::try_from(arrow_msg).unwrap()
            } else {
                unreachable!()
            }
        })
        .collect()
}

fn mono_points_classic(c: &mut Criterion) {
    fn generate_messages() -> Vec<LogMsg> {
        let timeline = Timeline::new_sequence("frame_nr");
        let pos_field_name = FieldName::from("pos");
        let radius_field_name = FieldName::from("radius");

        (0..NUM_POINTS)
            .flat_map(|i| {
                let obj_path = obj_path!("points", Index::Sequence(i as _));

                let mut time_point = TimePoint::default();
                time_point.insert(timeline, TimeInt::from(0));

                [
                    LogMsg::DataMsg(DataMsg {
                        msg_id: MsgId::ZERO,
                        time_point: time_point.clone(),
                        data_path: DataPath::new(obj_path.clone(), pos_field_name),
                        data: Data::Vec2(POS).into(),
                    }),
                    LogMsg::DataMsg(DataMsg {
                        msg_id: MsgId::ZERO,
                        time_point,
                        data_path: DataPath::new(obj_path, radius_field_name),
                        data: Data::Color(COLOR).into(),
                    }),
                ]
            })
            .collect()
    }

    {
        let mut group = c.benchmark_group("mono_points_classic");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_messages", |b| {
            b.iter(generate_messages);
        });
        let messages = generate_messages();
        group.bench_function("encode_log_msg", |b| {
            b.iter(|| encode_log_msgs(&messages));
        });

        group.bench_function("encode_total", |b| {
            b.iter(|| encode_log_msgs(&generate_messages()));
        });

        let encoded = encode_log_msgs(&messages);
        group.bench_function("decode_total", |b| {
            b.iter(|| {
                let decoded = decode_log_msgs(&encoded);
                assert_eq!(decoded.len(), messages.len());
                decoded
            });
        });
    }
}

fn mono_points_arrow(c: &mut Criterion) {
    fn generate_message_bundles() -> Vec<MsgBundle> {
        (0..NUM_POINTS)
            .map(|i| {
                try_build_msg_bundle2(
                    MsgId::ZERO,
                    obj_path!("points", Index::Sequence(i as _)),
                    [build_frame_nr(0.into())],
                    (build_some_point2d(1), build_some_colors(1)),
                )
                .unwrap()
            })
            .collect()
    }

    {
        let mut group = c.benchmark_group("mono_points_arrow");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_message_bundles", |b| {
            b.iter(generate_message_bundles);
        });
        let bundles = generate_message_bundles();
        group.bench_function("generate_messages", |b| {
            b.iter(|| generate_messages(&bundles));
        });
        let messages = generate_messages(&bundles);
        group.bench_function("encode_log_msg", |b| {
            b.iter(|| encode_log_msgs(&messages));
        });
        group.bench_function("encode_total", |b| {
            b.iter(|| encode_log_msgs(&generate_messages(&generate_message_bundles())));
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
                let bundles = decode_message_bundles(&messages);
                assert_eq!(bundles.len(), messages.len());
                bundles
            });
        });
        group.bench_function("decode_total", |b| {
            b.iter(|| decode_message_bundles(&decode_log_msgs(&encoded)));
        });
    }
}

fn batch_points_classic(c: &mut Criterion) {
    fn generate_messages() -> Vec<LogMsg> {
        let obj_path = obj_path!("points");
        let timeline = Timeline::new_sequence("frame_nr");
        let pos_field_name = FieldName::from("pos");
        let radius_field_name = FieldName::from("radius");

        let mut time_point = TimePoint::default();
        time_point.insert(timeline, TimeInt::from(0));

        vec![
            LogMsg::DataMsg(DataMsg {
                msg_id: MsgId::ZERO,
                time_point: time_point.clone(),
                data_path: DataPath::new(obj_path.clone(), pos_field_name),
                data: LoggedData::Batch {
                    indices: BatchIndex::SequentialIndex(NUM_POINTS),
                    data: DataVec::Vec2(vec![POS; NUM_POINTS]),
                },
            }),
            LogMsg::DataMsg(DataMsg {
                msg_id: MsgId::ZERO,
                time_point,
                data_path: DataPath::new(obj_path, radius_field_name),
                data: LoggedData::Batch {
                    indices: BatchIndex::SequentialIndex(NUM_POINTS),
                    data: DataVec::Color(vec![COLOR; NUM_POINTS]),
                },
            }),
        ]
    }

    {
        let mut group = c.benchmark_group("batch_points_classic");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_messages", |b| {
            b.iter(generate_messages);
        });
        let messages = generate_messages();
        group.bench_function("encode_log_msg", |b| {
            b.iter(|| encode_log_msgs(&messages));
        });

        group.bench_function("encode_total", |b| {
            b.iter(|| encode_log_msgs(&generate_messages()));
        });

        let encoded = encode_log_msgs(&messages);
        group.bench_function("decode_total", |b| {
            b.iter(|| {
                let decoded = decode_log_msgs(&encoded);
                assert_eq!(decoded.len(), messages.len());
                decoded
            });
        });
    }
}

fn batch_points_arrow(c: &mut Criterion) {
    fn generate_message_bundles() -> Vec<MsgBundle> {
        vec![try_build_msg_bundle2(
            MsgId::ZERO,
            obj_path!("points"),
            [build_frame_nr(0.into())],
            (
                build_some_point2d(NUM_POINTS),
                build_some_colors(NUM_POINTS),
            ),
        )
        .unwrap()]
    }

    {
        let mut group = c.benchmark_group("batch_points_arrow");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_message_bundles", |b| {
            b.iter(generate_message_bundles);
        });
        let bundles = generate_message_bundles();
        group.bench_function("generate_messages", |b| {
            b.iter(|| generate_messages(&bundles));
        });
        let messages = generate_messages(&bundles);
        group.bench_function("encode_log_msg", |b| {
            b.iter(|| encode_log_msgs(&messages));
        });
        group.bench_function("encode_total", |b| {
            b.iter(|| encode_log_msgs(&generate_messages(&generate_message_bundles())));
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
                let bundles = decode_message_bundles(&messages);
                assert_eq!(bundles.len(), messages.len());
                bundles
            });
        });
        group.bench_function("decode_total", |b| {
            b.iter(|| decode_message_bundles(&decode_log_msgs(&encoded)));
        });
    }
}
