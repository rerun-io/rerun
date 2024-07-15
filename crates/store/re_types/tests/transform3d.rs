use std::{collections::HashMap, f32::consts::TAU};

use re_types::{
    archetypes::Transform3D,
    components,
    datatypes::{
        self, Angle, Mat3x3, Rotation3D, RotationAxisAngle, Scale3D, TranslationRotationScale3D,
        Vec3D,
    },
    Archetype as _, AsComponents as _,
};

#[test]
fn roundtrip() {
    let all_expected = [
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationRotationScale(
                TranslationRotationScale3D {
                    translation: None,
                    rotation: None,
                    scale: None,
                    from_parent: false,
                },
            )),
            mat3x3: None,
            translation: None,
            axis_length: None,
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationRotationScale(
                TranslationRotationScale3D {
                    translation: None,
                    rotation: None,
                    scale: Some(Scale3D::Uniform(42.0)),
                    from_parent: true,
                },
            )),
            mat3x3: None,
            translation: Some(vec![Vec3D([1.0, 2.0, 3.0]).into()]),
            axis_length: None,
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationRotationScale(
                TranslationRotationScale3D {
                    translation: None,
                    rotation: Some(Rotation3D::AxisAngle(RotationAxisAngle {
                        axis: Vec3D([0.2, 0.2, 0.8]),
                        angle: Angle::Radians(0.5 * TAU),
                    })),
                    scale: None,
                    from_parent: false,
                },
            )),
            mat3x3: None,
            translation: Some(vec![Vec3D([1.0, 2.0, 3.0]).into()]),
            axis_length: None,
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationRotationScale(
                TranslationRotationScale3D {
                    translation: None,
                    rotation: Some(Rotation3D::AxisAngle(RotationAxisAngle {
                        axis: Vec3D([0.2, 0.2, 0.8]),
                        angle: Angle::Radians(0.5 * TAU),
                    })),
                    scale: Some(Scale3D::Uniform(42.0)),
                    from_parent: true,
                },
            )),
            mat3x3: None,
            translation: Some(vec![Vec3D([1.0, 2.0, 3.0]).into()]),
            axis_length: None,
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationRotationScale(
                TranslationRotationScale3D {
                    translation: None,
                    rotation: None,
                    scale: None,
                    from_parent: true,
                },
            )),
            mat3x3: None,
            translation: Some(vec![Vec3D([1.0, 2.0, 3.0]).into()]),
            axis_length: None,
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationRotationScale(
                TranslationRotationScale3D {
                    translation: None,
                    rotation: None,
                    scale: None,
                    from_parent: true,
                },
            )),
            mat3x3: Some(vec![
                Mat3x3([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]).into()
            ]),
            translation: None,
            axis_length: None,
        }, //
    ];

    let all_arch = [
        Transform3D::default(),
        Transform3D::from_translation_scale([1.0, 2.0, 3.0], Scale3D::Uniform(42.0)).from_parent(), //
        Transform3D::from_translation_rotation(
            [1.0, 2.0, 3.0],
            RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(0.5 * TAU)),
        ), //
        Transform3D::from_translation_rotation_scale(
            [1.0, 2.0, 3.0],
            RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(0.5 * TAU)),
            42.0,
        )
        .from_parent(),
        Transform3D::from_translation([1.0, 2.0, 3.0]).from_parent(),
        Transform3D::from_mat3x3([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]).from_parent(),
    ];

    let expected_extensions: HashMap<_, _> = [
        ("transform", vec!["rerun.components.Transform3D"]), //
    ]
    .into();

    for (expected, arch) in all_expected.into_iter().zip(all_arch) {
        similar_asserts::assert_eq!(expected, arch);

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

        let deserialized = Transform3D::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}

mod util;
