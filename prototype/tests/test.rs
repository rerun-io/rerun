use prototype::*;

#[test]
fn test_data_storage() {
    fn index_path(cam: &str, point: u64) -> IndexPathKey {
        IndexPathKey::new(im::vector![
            Index::String(cam.into()),
            Index::Sequence(point)
        ])
    }

    let pos_type_path = || {
        im::vector![
            TypePathComponent::Name("camera".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("point".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("pos".into()),
        ]
    };
    let radius_type_path = || {
        im::vector![
            TypePathComponent::Name("camera".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("point".into()),
            TypePathComponent::Index,
            TypePathComponent::Name("radius".into()),
        ]
    };

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

    assert_eq!(
        Scene3D::from_store(&store, &TimeQuery::LatestAt(TimeValue::Sequence(0))).points,
        vec![]
    );

    assert_eq!(
        Scene3D::from_store(&store, &TimeQuery::LatestAt(TimeValue::Sequence(1))).points,
        vec![Point3 {
            pos: &[1.0, 1.0, 1.0],
            radius: None
        }]
    );

    assert_eq!(
        Scene3D::from_store(&store, &TimeQuery::LatestAt(TimeValue::Sequence(2))).points,
        vec![Point3 {
            pos: &[1.0, 1.0, 1.0],
            radius: Some(1.0)
        }]
    );

    assert_eq!(
        Scene3D::from_store(&store, &TimeQuery::LatestAt(TimeValue::Sequence(3))).points,
        vec![Point3 {
            pos: &[3.0, 3.0, 3.0],
            radius: Some(1.0)
        }]
    );

    assert_eq!(
        Scene3D::from_store(&store, &TimeQuery::LatestAt(TimeValue::Sequence(4)))
            .points
            .len(),
        2
    );
}
