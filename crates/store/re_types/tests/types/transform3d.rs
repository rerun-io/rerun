use std::{collections::HashMap, f32::consts::TAU};

use re_types::{
    archetypes::Transform3D,
    components::{Scale3D, TransformRelation},
    datatypes::{Angle, Mat3x3, RotationAxisAngle, Vec3D},
    Archetype as _, AsComponents as _,
};

use crate::util;

#[test]
fn roundtrip() {
    let all_expected = [
        Transform3D::clear(),
        Transform3D {
            translation: Some(Vec3D([1.0, 2.0, 3.0]).into()),
            scale: Some(Scale3D::uniform(42.0)),
            relation: Some(TransformRelation::ChildFromParent),
            ..Transform3D::clear()
        }, //
        Transform3D {
            translation: Some([1.0, 2.0, 3.0].into()),
            rotation_axis_angle: Some(
                RotationAxisAngle {
                    axis: Vec3D([0.2, 0.2, 0.8]),
                    angle: Angle::from_radians(0.5 * TAU),
                }
                .into(),
            ),
            ..Transform3D::clear()
        }, //
        Transform3D {
            translation: Some(Vec3D([1.0, 2.0, 3.0]).into()),
            rotation_axis_angle: Some(
                RotationAxisAngle {
                    axis: Vec3D([0.2, 0.2, 0.8]),
                    angle: Angle::from_radians(0.5 * TAU),
                }
                .into(),
            ),
            scale: Some(Scale3D::uniform(42.0)),
            relation: Some(TransformRelation::ChildFromParent),
            ..Transform3D::clear()
        }, //
        Transform3D {
            translation: Some(Vec3D([1.0, 2.0, 3.0]).into()),
            relation: Some(TransformRelation::ChildFromParent),
            ..Transform3D::clear()
        }, //
        Transform3D {
            mat3x3: Some(Mat3x3([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]).into()),
            relation: Some(TransformRelation::ParentFromChild),
            ..Transform3D::clear()
        }, //
    ];

    let all_arch = [
        Transform3D::clear(),
        Transform3D::from_translation_scale([1.0, 2.0, 3.0], Scale3D::uniform(42.0))
            .with_relation(TransformRelation::ChildFromParent), //
        Transform3D::from_translation_rotation(
            [1.0, 2.0, 3.0],
            RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::from_radians(0.5 * TAU)),
        ), //
        Transform3D::from_translation_rotation_scale(
            [1.0, 2.0, 3.0],
            RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::from_radians(0.5 * TAU)),
            42.0,
        )
        .with_relation(TransformRelation::ChildFromParent),
        Transform3D::from_translation([1.0, 2.0, 3.0])
            .with_relation(TransformRelation::ChildFromParent),
        Transform3D::from_mat3x3([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]])
            .with_relation(TransformRelation::ParentFromChild),
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
            eprintln!("{} = {array:#?}", field.name());

            // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
            // has been fully replaced.
            if false {
                util::assert_extensions(
                    &**array,
                    expected_extensions[field.name().as_str()].as_slice(),
                );
            }
        }

        let deserialized = Transform3D::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}
