#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use criterion::{criterion_group, criterion_main, Criterion};
use itertools::Itertools;
use nohash_hasher::IntMap;

use re_data_store::{
    query::visit_type_data_3, DataStore, FieldName, Index, ObjPath, ObjPathComp, ObjTypePath,
    ObjTypePathComp, TimeInt, TimeQuery, Timeline,
};
use re_log_types::{
    obj_path, BatchIndex, Data, DataMsg, DataPath, DataVec, IndexHash, LoggedData, MsgId,
    ObjectType, TimePoint, TimeType,
};

#[cfg(not(debug_assertions))]
const NUM_FRAMES: i64 = 1_000;
#[cfg(not(debug_assertions))]
const NUM_POINTS: i64 = 1_000;

// `cargo test` also runs the benchmark setup code, so make sure they run quickly:
#[cfg(debug_assertions)]
const NUM_FRAMES: i64 = 1;
#[cfg(debug_assertions)]
const NUM_POINTS: i64 = 1;

fn timeline() -> Timeline {
    Timeline::new("frame", TimeType::Sequence)
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct Point {
    pos: [f32; 3],
    color: Option<[u8; 4]>,
}

fn do_query(obj_types: &IntMap<ObjTypePath, ObjectType>, data_store: &DataStore) -> Vec<Point> {
    let time_query = TimeQuery::LatestAt(NUM_FRAMES / 2);
    let timeline_store = data_store.get(&timeline()).unwrap();

    let mut points = Vec::new();

    for (obj_path, obj_store) in timeline_store.iter() {
        let _ = obj_types.get(obj_path.obj_type_path()).unwrap();
        visit_type_data_3(
            obj_store,
            &FieldName::from("pos"),
            &time_query,
            ("_visible", "color", "radius"),
            |_instance_index: Option<&IndexHash>,
             _time: i64,
             _msg_id: &MsgId,
             pos: &[f32; 3],
             _visible: Option<&bool>,
             color: Option<&[u8; 4]>,
             _radius: Option<&f32>| {
                points.push(Point {
                    pos: *pos,
                    color: color.cloned(),
                });
            },
        );
    }

    assert_eq!(points.len(), NUM_POINTS as usize);
    points
}

fn mono_data_messages() -> Vec<DataMsg> {
    let mut messages = Vec::with_capacity((NUM_FRAMES * NUM_POINTS * 3) as _);
    for frame_idx in 0..NUM_FRAMES {
        for point_idx in 0..NUM_POINTS {
            let mut time_point = TimePoint::default();
            time_point.insert(timeline(), TimeInt::from(frame_idx));

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
        }
    }
    messages
}

enum BatchType {
    Sequential,
    FullIndex,
}

fn batch_data_messages(batch_type: &BatchType) -> Vec<DataMsg> {
    let positions = vec![[1.0, 2.0, 3.0]; NUM_POINTS as usize];
    let colors = vec![[255; 4]; NUM_POINTS as usize];
    let indices = match batch_type {
        BatchType::Sequential => BatchIndex::SequentialIndex(NUM_POINTS as usize),
        BatchType::FullIndex => BatchIndex::FullIndex(
            (0..NUM_POINTS)
                .map(|pi| Index::Sequence(pi as _))
                .collect_vec(),
        ),
    };

    let mut messages = Vec::with_capacity((NUM_FRAMES * 3) as _);

    for frame_idx in 0..NUM_FRAMES {
        let mut time_point = TimePoint::default();
        time_point.insert(timeline(), TimeInt::from(frame_idx));

        let obj_path = obj_path!("points");

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
    }

    messages
}

fn insert_data(data_messages: &[DataMsg]) -> DataStore {
    let mut store = DataStore::default();
    for msg in data_messages {
        store.insert_data_msg(msg).unwrap();
    }
    store
}

fn obj_mono_points(c: &mut Criterion) {
    let data_messages = mono_data_messages();

    let mut obj_types = IntMap::default();
    obj_types.insert(
        ObjTypePath::new(vec![
            ObjTypePathComp::Name("points".into()),
            ObjTypePathComp::Index,
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
        let store = insert_data(&data_messages);
        group.bench_function("query", |b| {
            b.iter(|| do_query(&obj_types, &store));
        });
    }
}

fn obj_batch_points(c: &mut Criterion) {
    let data_messages = batch_data_messages(&BatchType::FullIndex);

    let mut obj_types = IntMap::default();
    obj_types.insert(
        ObjTypePath::new(vec![ObjTypePathComp::Name("points".into())]),
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
        let store = insert_data(&data_messages);
        group.bench_function("query", |b| {
            b.iter(|| do_query(&obj_types, &store));
        });
    }
}

fn obj_batch_points_sequential(c: &mut Criterion) {
    let data_messages = batch_data_messages(&BatchType::Sequential);

    let mut obj_types = IntMap::default();
    obj_types.insert(
        ObjTypePath::new(vec![ObjTypePathComp::Name("points".into())]),
        ObjectType::Point3D,
    );

    {
        let mut group = c.benchmark_group("obj_batch_points_sequential");
        group.throughput(criterion::Throughput::Elements(
            (NUM_POINTS * NUM_FRAMES) as _,
        ));
        group.bench_function("insert", |b| {
            b.iter(|| insert_data(&data_messages));
        });
    }

    {
        let mut group = c.benchmark_group("obj_batch_points_sequential");
        group.throughput(criterion::Throughput::Elements(NUM_POINTS as _));
        let store = insert_data(&data_messages);
        group.bench_function("query", |b| {
            b.iter(|| do_query(&obj_types, &store));
        });
    }
}

criterion_group!(
    benches,
    obj_mono_points,
    obj_batch_points,
    obj_batch_points_sequential
);
criterion_main!(benches);
