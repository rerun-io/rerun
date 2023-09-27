use std::collections::HashMap;

use re_types::{archetypes::Pinhole, components, Archetype as _, AsComponents as _};

mod util;

#[test]
fn roundtrip() {
    let expected = Pinhole {
        image_from_camera: components::PinholeProjection(
            [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]].into(),
        ),
        resolution: Some(components::Resolution([1.0, 2.0].into())),
        camera_xyz: Some(components::ViewCoordinates::RDF),
    };

    let arch = Pinhole::new([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]])
        .with_resolution([1.0, 2.0])
        .with_camera_xyz(components::ViewCoordinates::RDF);
    similar_asserts::assert_eq!(expected, arch);

    let expected_extensions: HashMap<_, _> = [
        ("half_sizes", vec!["rerun.components."]),
        ("centers", vec!["rerun.components.Position2D"]),
    ]
    .into();

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name);

        // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
        // has been fully replaced.
        if false {
            util::assert_extensions(
                &**array,
                expected_extensions[field.name.as_str()].as_slice(),
            );
        }
    }

    let deserialized = Pinhole::try_from_arrow(serialized).unwrap();
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
