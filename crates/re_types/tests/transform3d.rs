use std::{collections::HashMap, f32::consts::PI};

use re_types::{
    archetypes::Transform3D,
    components,
    datatypes::{
        self, Angle, Mat3x3, Rotation3D, RotationAxisAngle, Scale3D, TranslationAndMat3x3,
        TranslationRotationScale3D, Vec3D,
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
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationRotationScale(
                TranslationRotationScale3D {
                    translation: Some(Vec3D([1.0, 2.0, 3.0])),
                    rotation: None,
                    scale: Some(Scale3D::Uniform(42.0)),
                    from_parent: true,
                },
            )),
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationRotationScale(
                TranslationRotationScale3D {
                    translation: Some(Vec3D([1.0, 2.0, 3.0])),
                    rotation: Some(Rotation3D::AxisAngle(RotationAxisAngle {
                        axis: Vec3D([0.2, 0.2, 0.8]),
                        angle: Angle::Radians(PI),
                    })),
                    scale: None,
                    from_parent: false,
                },
            )),
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationRotationScale(
                TranslationRotationScale3D {
                    translation: Some(Vec3D([1.0, 2.0, 3.0])),
                    rotation: Some(Rotation3D::AxisAngle(RotationAxisAngle {
                        axis: Vec3D([0.2, 0.2, 0.8]),
                        angle: Angle::Radians(PI),
                    })),
                    scale: Some(Scale3D::Uniform(42.0)),
                    from_parent: true,
                },
            )),
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationAndMat3x3(
                TranslationAndMat3x3 {
                    translation: None,
                    mat3x3: None,
                    from_parent: false,
                },
            )),
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationAndMat3x3(
                TranslationAndMat3x3 {
                    translation: Some(Vec3D([1.0, 2.0, 3.0])),
                    mat3x3: None,
                    from_parent: true,
                },
            )),
        }, //
        Transform3D {
            transform: components::Transform3D(datatypes::Transform3D::TranslationAndMat3x3(
                TranslationAndMat3x3 {
                    translation: None,
                    mat3x3: Some(Mat3x3([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0])),
                    from_parent: true,
                },
            )),
        }, //
    ];

    let all_arch = [
        Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D::IDENTITY,
        )), //
        Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D {
                translation: Some([1.0, 2.0, 3.0].into()),
                scale: Some(Scale3D::Uniform(42.0)),
                ..Default::default()
            }
            .from_parent(),
        )), //
        Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D::rigid(
                [1.0, 2.0, 3.0],
                RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(PI)),
            ),
        )), //
        Transform3D::new(datatypes::Transform3D::TranslationRotationScale(
            TranslationRotationScale3D::affine(
                [1.0, 2.0, 3.0],
                RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(PI)),
                42.0,
            )
            .from_parent(),
        )), //
        Transform3D::new(datatypes::Transform3D::TranslationAndMat3x3(
            TranslationAndMat3x3::IDENTITY,
        )), //
        Transform3D::new(datatypes::Transform3D::TranslationAndMat3x3(
            TranslationAndMat3x3::translation([1.0, 2.0, 3.0]).from_parent(),
        )), //
        Transform3D::new(datatypes::Transform3D::TranslationAndMat3x3(
            TranslationAndMat3x3::rotation([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]])
                .from_parent(),
        )), //
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
