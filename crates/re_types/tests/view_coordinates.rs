use std::collections::HashMap;

use re_types::{
    archetypes::ViewCoordinates, components, view_coordinates::ViewDir, Archetype as _,
};

#[test]
fn roundtrip() {
    let expected = ViewCoordinates {
        xyz: components::ViewCoordinates::new(ViewDir::Right, ViewDir::Down, ViewDir::Forward),
    };

    let arch = ViewCoordinates::RDF;

    similar_asserts::assert_eq!(expected, arch);

    let expected_extensions: HashMap<_, _> =
        [("coordinates", vec!["rerun.components.ViewCoordinates"])].into();

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow();
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

    let deserialized = ViewCoordinates::try_from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

// ----------------------------------------------------------------------------

#[cfg(feature = "glam")]
#[test]
fn view_coordinates() {
    use glam::{vec3, Mat3};
    use re_types::view_coordinates::{Handedness, SignedAxis3};

    assert_eq!(ViewCoordinates::RUB.xyz.to_rub(), Mat3::IDENTITY);
    assert_eq!(ViewCoordinates::RUB.xyz.from_rub(), Mat3::IDENTITY);

    {
        assert!("UUDDLRLRBAStart"
            .parse::<components::ViewCoordinates>()
            .is_err());
        assert!("UUD".parse::<components::ViewCoordinates>().is_err());

        let rub = "RUB".parse::<components::ViewCoordinates>().unwrap();
        let bru = "BRU".parse::<components::ViewCoordinates>().unwrap();

        assert_eq!(rub, ViewCoordinates::RUB.xyz);

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

mod util;
