#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use re_log_types::{
    obj_path, BatchIndex, Data, DataMsg, DataPath, DataVec, FieldName, Index, LogMsg, LoggedData,
    MsgId, TimeInt, TimePoint, Timeline,
};

use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(not(debug_assertions))]
const NUM_POINTS: i64 = 10_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_POINTS: i64 = 1;

criterion_group!(
    benches,
    mono_points_classic,
    batch_points_classic,
    // mono_points_arrow,
    // batch_points_arrow,
);
criterion_main!(benches);

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
                        data: Data::Vec3([0.0, 1.0, 2.0]).into(),
                    }),
                    LogMsg::DataMsg(DataMsg {
                        msg_id: MsgId::ZERO,
                        time_point,
                        data_path: DataPath::new(obj_path, radius_field_name),
                        data: Data::F32(0.1).into(),
                    }),
                ]
            })
            .collect()
    }

    fn encode(messages: &[LogMsg]) -> Vec<u8> {
        let mut bytes = vec![];
        re_log_types::encoding::encode(messages.iter(), &mut bytes).unwrap();
        assert!(bytes.len() > messages.len());
        bytes
    }

    fn decode(mut bytes: &[u8]) -> Vec<LogMsg> {
        let messages = re_log_types::encoding::Decoder::new(&mut bytes)
            .unwrap()
            .collect::<anyhow::Result<Vec<LogMsg>>>()
            .unwrap();
        assert!(bytes.is_empty());
        messages
    }

    {
        let mut group = c.benchmark_group("mono_points_classic");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_messages", |b| {
            b.iter(generate_messages);
        });
        let messages = generate_messages();
        group.bench_function("encode", |b| {
            b.iter(|| encode(&messages));
        });
        let encoded = encode(&messages);
        group.bench_function("decode", |b| {
            b.iter(|| {
                let decoded = decode(&encoded);
                assert_eq!(decoded.len(), messages.len());
                decoded
            });
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
                    indices: BatchIndex::SequentialIndex(NUM_POINTS as _),
                    data: DataVec::Vec3(vec![[0.0, 1.0, 2.0]; NUM_POINTS as usize]),
                },
            }),
            LogMsg::DataMsg(DataMsg {
                msg_id: MsgId::ZERO,
                time_point,
                data_path: DataPath::new(obj_path, radius_field_name),
                data: LoggedData::Batch {
                    indices: BatchIndex::SequentialIndex(NUM_POINTS as _),
                    data: DataVec::F32(vec![0.1; NUM_POINTS as usize]),
                },
            }),
        ]
    }

    fn encode(messages: &[LogMsg]) -> Vec<u8> {
        let mut bytes = vec![];
        re_log_types::encoding::encode(messages.iter(), &mut bytes).unwrap();
        assert!(bytes.len() > messages.len());
        bytes
    }

    fn decode(mut bytes: &[u8]) -> Vec<LogMsg> {
        let messages = re_log_types::encoding::Decoder::new(&mut bytes)
            .unwrap()
            .collect::<anyhow::Result<Vec<LogMsg>>>()
            .unwrap();
        assert!(bytes.is_empty());
        messages
    }

    {
        let mut group = c.benchmark_group("batch_points_classic");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        group.bench_function("generate_messages", |b| {
            b.iter(generate_messages);
        });
        let messages = generate_messages();
        group.bench_function("encode", |b| {
            b.iter(|| encode(&messages));
        });
        let encoded = encode(&messages);
        group.bench_function("decode", |b| {
            b.iter(|| {
                let decoded = decode(&encoded);
                assert_eq!(decoded.len(), messages.len());
                decoded
            });
        });
    }
}
