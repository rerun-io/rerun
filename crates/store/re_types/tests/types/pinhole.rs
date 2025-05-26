use re_types::{
    archetypes::Pinhole, components, Archetype as _, AsComponents as _, ComponentBatch as _,
};

#[test]
fn roundtrip() {
    let expected = Pinhole {
        image_from_camera: components::PinholeProjection(
            [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]].into(),
        )
        .serialized(Pinhole::descriptor_image_from_camera()),
        resolution: components::Resolution([1.0, 2.0].into())
            .serialized(Pinhole::descriptor_resolution()),
        camera_xyz: components::ViewCoordinates::RDF.serialized(Pinhole::descriptor_camera_xyz()),
        image_plane_distance: None,
    };

    let arch = Pinhole::new([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]])
        .with_resolution([1.0, 2.0])
        .with_camera_xyz(components::ViewCoordinates::RDF);
    similar_asserts::assert_eq!(expected, arch);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = Pinhole::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

#[test]
fn from_focal_length_and_resolution() {
    assert_eq!(
        Pinhole::from_focal_length_and_resolution([1.0, 2.0], [3.0, 4.0]),
        Pinhole::new([[1.0, 0.0, 0.0], [0.0, 2.0, 0.0], [1.5, 2.0, 1.0]])
            .with_resolution([3.0, 4.0])
    );
}
