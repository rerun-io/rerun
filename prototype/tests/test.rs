use prototype::*;

#[test]
fn test_data_storage() {
    fn points_at(store: &DataStore, frame: i64) -> Vec<Point3<'_>> {
        let time_query = TimeQuery::LatestAt(TimeValue::Sequence(frame));
        let mut points = Scene3D::from_store(store, &time_query).points;
        points.sort_by(|a, b| a.partial_cmp(b).unwrap());
        points
    }

    fn index_path(cam: &str, point: u64) -> IndexPathKey {
        IndexPathKey::new(im::vector![
            Index::String(cam.into()),
            Index::Sequence(point)
        ])
    }

    fn pos_type_path() -> TypePath {
        im::vector![
            TypePathComponent::Name("camera".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("point".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("pos".into()),
        ]
    }
    fn radius_type_path() -> TypePath {
        im::vector![
            TypePathComponent::Name("camera".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("point".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("radius".into()),
        ]
    }

    let mut store = DataStore::default();

    store.insert_individual::<[f32; 3]>(
        pos_type_path(),
        index_path("left", 0),
        TimeValue::Sequence(1),
        [1.0, 1.0, 1.0],
    );

    store.insert_individual::<[f32; 3]>(
        pos_type_path(),
        index_path("left", 0),
        TimeValue::Sequence(3),
        [3.0, 3.0, 3.0],
    );

    store.insert_individual::<f32>(
        radius_type_path(),
        index_path("left", 0),
        TimeValue::Sequence(2),
        1.0,
    );

    store.insert_individual::<[f32; 3]>(
        pos_type_path(),
        index_path("left", 1),
        TimeValue::Sequence(4),
        [4.0, 4.0, 4.0],
    );

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
}

#[test]
fn test_batches() {
    fn index_path_prefix(cam: &str) -> IndexPathKey {
        IndexPathKey::new(im::vector![Index::String(cam.into())])
    }

    fn prim() -> TypePath {
        im::vector![
            TypePathComponent::Name("camera".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("point".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("pos".into()),
        ]
    }
    fn sibling() -> TypePath {
        im::vector![
            TypePathComponent::Name("camera".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("point".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("label".into()),
        ]
    }

    fn values(store: &DataStore, frame: i64) -> Vec<(i32, Option<&str>)> {
        let time_query = TimeQuery::LatestAt(TimeValue::Sequence(frame));
        let mut values = vec![];
        visit_data_and_siblings(store, &time_query, &prim(), ("label",), |prim, sibling| {
            values.push((*prim, sibling.copied()));
        });
        values.sort();
        values
    }

    fn index(seq: u64) -> IndexKey {
        IndexKey::new(Index::Sequence(seq))
    }

    let mut store = DataStore::default();

    store.insert_batch(
        prim(),
        index_path_prefix("left"),
        TimeValue::Sequence(1),
        [
            (index(0), 0_i32),
            (index(1), 1_i32),
            (index(2), 2_i32),
            (index(3), 3_i32),
        ]
        .into_iter(),
    );
    store.insert_batch(
        prim(),
        index_path_prefix("right"),
        TimeValue::Sequence(2),
        [
            (index(0), 1_000_i32),
            (index(1), 1_001_i32),
            (index(2), 1_002_i32),
            (index(3), 1_003_i32),
        ]
        .into_iter(),
    );
    store.insert_batch(
        prim(),
        index_path_prefix("left"),
        TimeValue::Sequence(3),
        [
            // 0, 1 omitted = dropped
            (index(2), 22_i32),
            (index(3), 33_i32),
        ]
        .into_iter(),
    );
    store.insert_batch(
        sibling(),
        index_path_prefix("left"),
        TimeValue::Sequence(4),
        [(index(1), "one"), (index(2), "two")].into_iter(),
    );
    store.insert_batch(
        sibling(),
        index_path_prefix("right"),
        TimeValue::Sequence(5),
        [
            (index(0), "r0"),
            (index(1), "r1"),
            (index(2), "r2"),
            (index(3), "r3"),
            (index(4), "r4"), // has no point yet
        ]
        .into_iter(),
    );
    store.insert_batch(
        prim(),
        index_path_prefix("right"),
        TimeValue::Sequence(6),
        [
            (index(3), 1_003_i32),
            (index(4), 1_004_i32),
            (index(5), 1_005_i32),
        ]
        .into_iter(),
    );

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
            (22, Some("two")),
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
            (22, Some("two")),
            (33, None),
            (1_000, Some("r0")),
            (1_001, Some("r1")),
            (1_002, Some("r2")),
            (1_003, Some("r3")),
        ]
    );
    assert_eq!(
        values(&store, 6),
        vec![
            (22, Some("two")),
            (33, None),
            (1_003, Some("r3")),
            (1_004, Some("r4")),
            (1_005, None),
        ]
    );
}
