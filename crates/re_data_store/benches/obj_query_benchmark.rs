#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, criterion_main, Criterion};

use itertools::Itertools;
use nohash_hasher::IntMap;
use re_data_store::*;
use re_log_types::*;

const NUM_FRAMES: i64 = 1_000;
const NUM_POINTS: i64 = 1_000;

fn time_source() -> TimeSource {
    TimeSource::new("frame", TimeType::Sequence)
}

fn do_query<'s>(
    obj_types: &IntMap<ObjTypePath, ObjectType>,
    data_store: &'s LogDataStore,
) -> Objects<'s> {
    let time_query = TimeQuery::LatestAt(NUM_FRAMES / 2);

    let (_, store) = data_store.get(&time_source()).unwrap();

    let mut objects = Objects::default();
    for (obj_type_path, obj_type) in obj_types {
        objects.query_object(store, &time_query, obj_type_path, *obj_type);
    }

    assert_eq!(objects.point3d.len(), NUM_POINTS as usize);

    objects
}

fn mono_data_messages() -> Vec<DataMsg> {
    let mut messages = Vec::with_capacity((NUM_FRAMES * NUM_POINTS * 3) as _);
    for frame_idx in 0..NUM_FRAMES {
        for point_idx in 0..NUM_POINTS {
            let mut time_point = TimePoint::default();
            time_point.0.insert(time_source(), TimeInt::from(frame_idx));

            let obj_path = obj_path!("points", Index::Sequence(point_idx as _));

            messages.push(DataMsg {
                msg_id: MsgId::random(),
                time_point: time_point.clone(),
                data_path: DataPath::new(obj_path.clone(), "pos".into()),
                data: LoggedData::Single(Data::Vec3([1.0, 2.0, 3.0])),
            });
            messages.push(DataMsg {
                msg_id: MsgId::random(),
                time_point: time_point.clone(),
                data_path: DataPath::new(obj_path.clone(), "color".into()),
                data: LoggedData::Single(Data::Color([255, 255, 255, 255])),
            });
            messages.push(DataMsg {
                msg_id: MsgId::random(),
                time_point: time_point.clone(),
                data_path: DataPath::new(obj_path, "space".into()),
                data: LoggedData::Single(Data::Space("world".into())),
            });
        }
    }
    messages
}

fn batch_data_messages() -> Vec<DataMsg> {
    let positions = vec![[1.0, 2.0, 3.0]; NUM_POINTS as usize];
    let colors = vec![[255; 4]; NUM_POINTS as usize];
    let indices = (0..NUM_POINTS)
        .map(|pi| Index::Sequence(pi as _))
        .collect_vec();

    let mut messages = Vec::with_capacity((NUM_FRAMES * 3) as _);

    for frame_idx in 0..NUM_FRAMES {
        let mut time_point = TimePoint::default();
        time_point.0.insert(time_source(), TimeInt::from(frame_idx));

        let obj_path = obj_path!("points", Index::Placeholder);

        messages.push(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path.clone(), "pos".into()),
            data: LoggedData::Batch {
                indices: indices.clone(),
                data: DataVec::Vec3(positions.clone()),
            },
        });
        messages.push(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path.clone(), "color".into()),
            data: LoggedData::Batch {
                indices: indices.clone(),
                data: DataVec::Color(colors.clone()),
            },
        });
        messages.push(DataMsg {
            msg_id: MsgId::random(),
            time_point: time_point.clone(),
            data_path: DataPath::new(obj_path, "space".into()),
            data: LoggedData::BatchSplat(Data::Space("world".into())),
        });
    }

    messages
}

fn insert_data(data_messages: &[DataMsg]) -> LogDataStore {
    let mut data_store = LogDataStore::default();
    for msg in data_messages {
        data_store.insert(msg).unwrap();
    }
    data_store
}

fn obj_mono_points(c: &mut Criterion) {
    let data_messages = mono_data_messages();

    let mut obj_types = IntMap::default();
    obj_types.insert(
        ObjTypePath::new(vec![
            TypePathComp::String("points".into()),
            TypePathComp::Index,
        ]),
        ObjectType::Point3D,
    );

    {
        let mut group = c.benchmark_group("obj_mono_points");
        group.throughput(criterion::Throughput::Elements(
            (NUM_POINTS * NUM_FRAMES) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_data(&data_messages));
        });
    }

    {
        let mut group = c.benchmark_group("obj_mono_points");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let data_store = insert_data(&data_messages);
        group.bench_function("query", |b| {
            b.iter(|| do_query(&obj_types, &data_store));
        });
    }
}

fn obj_batch_points(c: &mut Criterion) {
    let data_messages = batch_data_messages();

    let mut obj_types = IntMap::default();
    obj_types.insert(
        ObjTypePath::new(vec![
            TypePathComp::String("points".into()),
            TypePathComp::Index,
        ]),
        ObjectType::Point3D,
    );

    {
        let mut group = c.benchmark_group("obj_batch_points");
        group.throughput(criterion::Throughput::Elements(
            (NUM_POINTS * NUM_FRAMES) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_data(&data_messages));
        });
    }

    {
        let mut group = c.benchmark_group("obj_batch_points");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let data_store = insert_data(&data_messages);
        group.bench_function("query", |b| {
            b.iter(|| do_query(&obj_types, &data_store));
        });
    }
}

criterion_group!(benches, obj_mono_points, obj_batch_points);
criterion_main!(benches);
