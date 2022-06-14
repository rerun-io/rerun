use data_store::*;
use log_types::{FieldName, IndexKey, LogId};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Time(i64);

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Point3<'s> {
    pub pos: &'s [f32; 3],
    pub radius: Option<f32>,
}

pub fn points_from_store<'store, Time: 'static + Copy + Ord>(
    store: &'store ObjStore<Time>,
    time_query: &TimeQuery<Time>,
) -> Vec<Point3<'store>> {
    let mut points = vec![];
    visit_type_data_1(
        store,
        &FieldName::from("pos"),
        time_query,
        ("radius",),
        |_object_path, _log_id: &LogId, pos: &[f32; 3], radius: Option<&f32>| {
            points.push(Point3 {
                pos,
                radius: radius.copied(),
            });
        },
    );
    points
}

fn batch<T, const N: usize>(batch: [(IndexKey, T); N]) -> Batch<T> {
    let batch: nohash_hasher::IntMap<IndexKey, T> = batch.into_iter().collect();
    std::sync::Arc::new(batch)
}

fn id() -> LogId {
    LogId::random()
}

fn s(s: &str) -> String {
    s.into()
}

#[test]
fn test_singular() -> data_store::Result<()> {
    fn points_at(store: &TypePathDataStore<Time>, frame: i64) -> Vec<Point3<'_>> {
        let time_query = TimeQuery::LatestAt(Time(frame));
        let obj_store = store.get(&obj_type_path()).unwrap();
        let mut points: Vec<_> = points_from_store(obj_store, &time_query);
        points.sort_by(|a, b| a.partial_cmp(b).unwrap());
        points
    }

    fn obj_type_path() -> ObjTypePath {
        ObjTypePath::new(vec![
            TypePathComp::String("camera".into()),
            TypePathComp::Index,
            TypePathComp::String("point".into()),
            TypePathComp::Index,
        ])
    }
    fn obj_data_path(cam: &str, point: u64) -> ObjPath {
        ObjPathBuilder::new(vec![
            ObjPathComp::String("camera".into()),
            ObjPathComp::Index(Index::String(cam.into())),
            ObjPathComp::String("point".into()),
            ObjPathComp::Index(Index::Sequence(point)),
        ])
        .into()
    }

    let mut store = TypePathDataStore::default();

    store.insert_individual::<[f32; 3]>(
        obj_data_path("left", 0),
        "pos".into(),
        Time(1),
        id(),
        [1.0, 1.0, 1.0],
    )?;

    store.insert_individual::<[f32; 3]>(
        obj_data_path("left", 0),
        "pos".into(),
        Time(3),
        id(),
        [3.0, 3.0, 3.0],
    )?;

    store.insert_individual::<f32>(
        obj_data_path("left", 0),
        "radius".into(),
        Time(2),
        id(),
        1.0,
    )?;

    store.insert_individual::<[f32; 3]>(
        obj_data_path("left", 1),
        "pos".into(),
        Time(4),
        id(),
        [4.0, 4.0, 4.0],
    )?;

    assert_eq!(points_at(&store, 0), vec![]);

    assert_eq!(
        points_at(&store, 1),
        vec![Point3 {
            pos: &[1.0, 1.0, 1.0],
            radius: None
        }]
    );

    assert_eq!(
        points_at(&store, 2),
        vec![Point3 {
            pos: &[1.0, 1.0, 1.0],
            radius: Some(1.0)
        }]
    );

    assert_eq!(
        points_at(&store, 3),
        vec![Point3 {
            pos: &[3.0, 3.0, 3.0],
            radius: Some(1.0)
        }]
    );

    assert_eq!(
        points_at(&store, 4),
        vec![
            Point3 {
                pos: &[3.0, 3.0, 3.0],
                radius: Some(1.0)
            },
            Point3 {
                pos: &[4.0, 4.0, 4.0],
                radius: None
            }
        ]
    );

    Ok(())
}

#[test]
fn test_batches() -> data_store::Result<()> {
    fn index_path_prefix(cam: &str) -> IndexPath {
        IndexPath::new(vec![Index::String(cam.into()), Index::Placeholder])
    }

    fn obj_type_path() -> ObjTypePath {
        ObjTypePath::new(vec![
            TypePathComp::String("camera".into()),
            TypePathComp::Index,
            TypePathComp::String("point".into()),
            TypePathComp::Index,
        ])
    }

    fn values(store: &TypePathDataStore<Time>, frame: i64) -> Vec<(i32, Option<String>)> {
        let obj_store = store.get(&obj_type_path()).unwrap();
        let time_query = TimeQuery::LatestAt(Time(frame));
        let mut values = vec![];
        visit_type_data_1(
            obj_store,
            &FieldName::new("pos"),
            &time_query,
            ("label",),
            |_object_path, _log_id, prim: &i32, sibling: Option<&String>| {
                values.push((*prim, sibling.cloned()));
            },
        );
        values.sort();
        values
    }

    fn index(seq: u64) -> IndexKey {
        IndexKey::new(Index::Sequence(seq))
    }

    let mut store = TypePathDataStore::default();

    store.insert_batch(
        obj_type_path(),
        index_path_prefix("left"),
        "pos".into(),
        Time(1),
        id(),
        batch([
            (index(0), 0_i32),
            (index(1), 1_i32),
            (index(2), 2_i32),
            (index(3), 3_i32),
        ]),
    )?;
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("right"),
        "pos".into(),
        Time(2),
        id(),
        batch([
            (index(0), 1_000_i32),
            (index(1), 1_001_i32),
            (index(2), 1_002_i32),
            (index(3), 1_003_i32),
        ]),
    )?;
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("left"),
        "pos".into(),
        Time(3),
        id(),
        batch([
            // 0, 1 omitted = dropped
            (index(2), 22_i32),
            (index(3), 33_i32),
        ]),
    )?;
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("left"),
        "label".into(),
        Time(4),
        id(),
        batch([(index(1), s("one")), (index(2), s("two"))]),
    )?;
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("right"),
        "label".into(),
        Time(5),
        id(),
        batch([
            (index(0), s("r0")),
            (index(1), s("r1")),
            (index(2), s("r2")),
            (index(3), s("r3")),
            (index(4), s("r4")), // has no point yet
        ]),
    )?;
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("right"),
        "pos".into(),
        Time(6),
        id(),
        batch([
            (index(3), 1_003_i32),
            (index(4), 1_004_i32),
            (index(5), 1_005_i32),
        ]),
    )?;
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("right"),
        "label".into(),
        Time(7),
        id(),
        batch([
            (index(3), s("r3_new")),
            // omitted = replaced
        ]),
    )?;

    assert_eq!(values(&store, 0), vec![]);
    assert_eq!(
        values(&store, 1),
        vec![(0, None), (1, None), (2, None), (3, None),]
    );
    assert_eq!(
        values(&store, 2),
        vec![
            (0, None),
            (1, None),
            (2, None),
            (3, None),
            (1_000, None),
            (1_001, None),
            (1_002, None),
            (1_003, None),
        ]
    );
    assert_eq!(
        values(&store, 3),
        vec![
            (22, None),
            (33, None),
            (1_000, None),
            (1_001, None),
            (1_002, None),
            (1_003, None),
        ]
    );
    assert_eq!(
        values(&store, 4),
        vec![
            (22, Some(s("two"))),
            (33, None),
            (1_000, None),
            (1_001, None),
            (1_002, None),
            (1_003, None),
        ]
    );
    assert_eq!(
        values(&store, 5),
        vec![
            (22, Some(s("two"))),
            (33, None),
            (1_000, Some(s("r0"))),
            (1_001, Some(s("r1"))),
            (1_002, Some(s("r2"))),
            (1_003, Some(s("r3"))),
        ]
    );
    assert_eq!(
        values(&store, 6),
        vec![
            (22, Some(s("two"))),
            (33, None),
            (1_003, Some(s("r3"))),
            (1_004, Some(s("r4"))),
            (1_005, None),
        ]
    );
    assert_eq!(
        values(&store, 7),
        vec![
            (22, Some(s("two"))),
            (33, None),
            (1_003, Some(s("r3_new"))),
            (1_004, None),
            (1_005, None),
        ]
    );

    Ok(())
}

#[test]
fn test_batched_and_individual() -> data_store::Result<()> {
    fn index_path_prefix(cam: &str) -> IndexPath {
        IndexPath::new(vec![Index::String(cam.into()), Index::Placeholder])
    }

    fn obj_type_path() -> ObjTypePath {
        ObjTypePath::new(vec![
            TypePathComp::String("camera".into()),
            TypePathComp::Index,
            TypePathComp::String("point".into()),
            TypePathComp::Index,
        ])
    }
    fn obj_path(cam: &str, point: u64) -> ObjPath {
        ObjPathBuilder::new(vec![
            ObjPathComp::String("camera".into()),
            ObjPathComp::Index(Index::String(cam.into())),
            ObjPathComp::String("point".into()),
            ObjPathComp::Index(Index::Sequence(point)),
        ])
        .into()
    }

    fn values(store: &TypePathDataStore<Time>, frame: i64) -> Vec<(i32, Option<String>)> {
        let obj_store = store.get(&obj_type_path()).unwrap();
        let time_query = TimeQuery::LatestAt(Time(frame));
        let mut values = vec![];
        visit_type_data_1(
            obj_store,
            &FieldName::new("pos"),
            &time_query,
            ("label",),
            |_object_path, _log_id, prim, sibling| {
                values.push((*prim, sibling.cloned()));
            },
        );
        values.sort();
        values
    }

    fn index(seq: u64) -> IndexKey {
        IndexKey::new(Index::Sequence(seq))
    }

    let mut store = TypePathDataStore::default();

    store.insert_batch(
        obj_type_path(),
        index_path_prefix("left"),
        "pos".into(),
        Time(1),
        id(),
        batch([
            (index(0), 0_i32),
            (index(1), 1_i32),
            (index(2), 2_i32),
            (index(3), 3_i32),
        ]),
    )?;
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("right"),
        "pos".into(),
        Time(2),
        id(),
        batch([
            (index(0), 1_000_i32),
            (index(1), 1_001_i32),
            (index(2), 1_002_i32),
            (index(3), 1_003_i32),
        ]),
    )?;
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("left"),
        "pos".into(),
        Time(3),
        id(),
        batch([
            // 0, 1 omitted = dropped
            (index(2), 22_i32),
            (index(3), 33_i32),
        ]),
    )?;
    store.insert_individual(obj_path("left", 1), "label".into(), Time(4), id(), s("one"))?;
    store.insert_individual(obj_path("left", 2), "label".into(), Time(4), id(), s("two"))?;
    for (index, value) in [
        (0, s("r0")),
        (1, s("r1")),
        (2, s("r2")),
        (3, s("r3")),
        (4, s("r4")), // has no point yet
    ] {
        store.insert_individual(
            obj_path("right", index),
            "label".into(),
            Time(5),
            id(),
            value,
        )?;
    }
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("right"),
        "pos".into(),
        Time(6),
        id(),
        batch([
            (index(3), 1_003_i32),
            (index(4), 1_004_i32),
            (index(5), 1_005_i32),
        ]),
    )?;
    store.insert_individual(obj_path("right", 5), "label".into(), Time(7), id(), s("r5"))?;

    assert_eq!(values(&store, 0), vec![]);
    assert_eq!(
        values(&store, 1),
        vec![(0, None), (1, None), (2, None), (3, None),]
    );
    assert_eq!(
        values(&store, 2),
        vec![
            (0, None),
            (1, None),
            (2, None),
            (3, None),
            (1_000, None),
            (1_001, None),
            (1_002, None),
            (1_003, None),
        ]
    );
    assert_eq!(
        values(&store, 3),
        vec![
            (22, None),
            (33, None),
            (1_000, None),
            (1_001, None),
            (1_002, None),
            (1_003, None),
        ]
    );
    assert_eq!(
        values(&store, 4),
        vec![
            (22, Some(s("two"))),
            (33, None),
            (1_000, None),
            (1_001, None),
            (1_002, None),
            (1_003, None),
        ]
    );
    assert_eq!(
        values(&store, 5),
        vec![
            (22, Some(s("two"))),
            (33, None),
            (1_000, Some(s("r0"))),
            (1_001, Some(s("r1"))),
            (1_002, Some(s("r2"))),
            (1_003, Some(s("r3"))),
        ]
    );
    assert_eq!(
        values(&store, 6),
        vec![
            (22, Some(s("two"))),
            (33, None),
            (1_003, Some(s("r3"))),
            (1_004, Some(s("r4"))),
            (1_005, None),
        ]
    );
    assert_eq!(
        values(&store, 7),
        vec![
            (22, Some(s("two"))),
            (33, None),
            (1_003, Some(s("r3"))),
            (1_004, Some(s("r4"))),
            (1_005, Some(s("r5"))),
        ]
    );

    Ok(())
}

#[test]
fn test_individual_and_batched() -> data_store::Result<()> {
    fn index_path_prefix(cam: &str) -> IndexPath {
        IndexPath::new(vec![Index::String(cam.into()), Index::Placeholder])
    }

    fn obj_type_path() -> ObjTypePath {
        ObjTypePath::new(vec![
            TypePathComp::String("camera".into()),
            TypePathComp::Index,
            TypePathComp::String("point".into()),
            TypePathComp::Index,
        ])
    }
    fn obj_path(cam: &str, point: u64) -> ObjPath {
        ObjPathBuilder::new(vec![
            ObjPathComp::String("camera".into()),
            ObjPathComp::Index(Index::String(cam.into())),
            ObjPathComp::String("point".into()),
            ObjPathComp::Index(Index::Sequence(point)),
        ])
        .into()
    }

    fn values(store: &TypePathDataStore<Time>, frame: i64) -> Vec<(i32, Option<String>)> {
        let obj_store = store.get(&obj_type_path()).unwrap();
        let time_query = TimeQuery::LatestAt(Time(frame));
        let mut values = vec![];
        visit_type_data_1(
            obj_store,
            &FieldName::new("pos"),
            &time_query,
            ("label",),
            |_object_path, _log_id, prim, sibling| {
                values.push((*prim, sibling.cloned()));
            },
        );
        values.sort();
        values
    }

    fn index(seq: u64) -> IndexKey {
        IndexKey::new(Index::Sequence(seq))
    }

    let mut store = TypePathDataStore::default();

    store.insert_individual(obj_path("left", 0), "pos".into(), Time(1), id(), 0_i32)?;
    store.insert_individual(obj_path("left", 1), "pos".into(), Time(2), id(), 1_i32)?;
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("left"),
        "label".into(),
        Time(3),
        id(),
        batch([(index(1), s("one")), (index(2), s("two"))]),
    )?;
    store.insert_individual(obj_path("left", 2), "pos".into(), Time(4), id(), 2_i32)?;
    store.insert_individual(obj_path("left", 3), "pos".into(), Time(4), id(), 3_i32)?;
    store.insert_batch(
        obj_type_path(),
        index_path_prefix("left"),
        "label".into(),
        Time(5),
        id(),
        batch([(index(2), s("two")), (index(3), s("three"))]),
    )?;

    assert_eq!(values(&store, 0), vec![]);
    assert_eq!(values(&store, 1), vec![(0, None)]);
    assert_eq!(values(&store, 2), vec![(0, None), (1, None)]);
    assert_eq!(values(&store, 3), vec![(0, None), (1, Some(s("one")))]);
    assert_eq!(
        values(&store, 4),
        vec![
            (0, None),
            (1, Some(s("one"))),
            (2, Some(s("two"))),
            (3, None)
        ]
    );
    assert_eq!(
        values(&store, 5),
        vec![
            (0, None),
            (1, None),
            (2, Some(s("two"))),
            (3, Some(s("three")))
        ]
    );

    Ok(())
}
