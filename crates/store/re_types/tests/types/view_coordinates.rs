use re_types::{
    archetypes::ViewCoordinates, components, view_coordinates::ViewDir, Archetype as _,
    AsComponents as _, ComponentBatch,
};

#[test]
fn roundtrip() {
    let expected = ViewCoordinates {
        xyz: components::ViewCoordinates::new(ViewDir::Right, ViewDir::Down, ViewDir::Forward)
            .serialized()
            .map(|xyz| xyz.with_descriptor_override(ViewCoordinates::descriptor_xyz())),
    };

    let arch = ViewCoordinates::RDF();

    similar_asserts::assert_eq!(expected, arch);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = ViewCoordinates::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

// ----------------------------------------------------------------------------

#[cfg(feature = "glam")]
#[test]
fn view_coordinates() {
    use glam::{vec3, Mat3};
    use re_types::view_coordinates::{Handedness, SignedAxis3};

    let rub_component =
        components::ViewCoordinates::new(ViewDir::Right, ViewDir::Up, ViewDir::Back);
    assert_eq!(rub_component.to_rub(), Mat3::IDENTITY);
    assert_eq!(rub_component.from_rub(), Mat3::IDENTITY);

    {
        assert!("UUDDLRLRBAStart"
            .parse::<components::ViewCoordinates>()
            .is_err());
        assert!("UUD".parse::<components::ViewCoordinates>().is_err());

        let rub = "RUB".parse::<components::ViewCoordinates>().unwrap();
        let bru = "BRU".parse::<components::ViewCoordinates>().unwrap();

        assert_eq!(rub, rub_component);

        assert_eq!(rub.to_rub(), Mat3::IDENTITY);
        assert_eq!(
            bru.to_rub(),
            Mat3::from_cols_array_2d(&[[0., 0., 1.], [1., 0., 0.], [0., 1., 0.]])
        );
        assert_eq!(bru.to_rub() * vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0));
    }

    {
        let cardinal_direction = [
            SignedAxis3::POSITIVE_X,
            SignedAxis3::NEGATIVE_X,
            SignedAxis3::POSITIVE_Y,
            SignedAxis3::NEGATIVE_Y,
            SignedAxis3::POSITIVE_Z,
            SignedAxis3::NEGATIVE_Z,
        ];

        for axis in cardinal_direction {
            for handedness in [Handedness::Right, Handedness::Left] {
                let system = components::ViewCoordinates::from_up_and_handedness(axis, handedness);
                assert_eq!(system.handedness(), Ok(handedness));

                let det = system.to_rub().determinant();
                assert!(det == -1.0 || det == 0.0 || det == 1.0);

                let short = system.describe_short();
                assert_eq!(short.parse(), Ok(system));
            }
        }
    }
}
