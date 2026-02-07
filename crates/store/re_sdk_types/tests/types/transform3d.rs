use std::f32::consts::TAU;

use re_sdk_types::archetypes::Transform3D;
use re_sdk_types::components::{
    RotationAxisAngle, Scale3D, TransformMat3x3, TransformRelation, Translation3D,
};
use re_sdk_types::datatypes::Angle;
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _};

#[test]
fn roundtrip() {
    let translation_serialized =
        Translation3D::new(1.0, 2.0, 3.0).serialized(Transform3D::descriptor_translation());
    let scale_serialized = Scale3D::uniform(42.0).serialized(Transform3D::descriptor_scale());

    let mat3x3_serialized = TransformMat3x3::from([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0])
        .serialized(Transform3D::descriptor_mat3x3());
    let rotation_axis_angle_serialized =
        RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::from_radians(0.5 * TAU))
            .serialized(Transform3D::descriptor_rotation_axis_angle());
    let relation_child_from_parent_serialized =
        TransformRelation::ChildFromParent.serialized(Transform3D::descriptor_relation());
    let relation_parent_from_child_serialized =
        TransformRelation::ParentFromChild.serialized(Transform3D::descriptor_relation());

    let all_expected = [
        Transform3D::clear_fields(),
        Transform3D {
            translation: translation_serialized.clone(),
            scale: scale_serialized.clone(),
            relation: relation_child_from_parent_serialized.clone(),
            ..Transform3D::default()
        }, //
        Transform3D {
            translation: translation_serialized.clone(),
            rotation_axis_angle: rotation_axis_angle_serialized.clone(),
            ..Transform3D::default()
        }, //
        Transform3D {
            translation: translation_serialized.clone(),
            rotation_axis_angle: rotation_axis_angle_serialized.clone(),
            scale: scale_serialized.clone(),
            relation: relation_child_from_parent_serialized.clone(),
            ..Transform3D::default()
        }, //
        Transform3D {
            translation: translation_serialized.clone(),
            relation: relation_child_from_parent_serialized.clone(),
            ..Transform3D::default()
        }, //
        Transform3D {
            mat3x3: mat3x3_serialized.clone(),
            relation: relation_parent_from_child_serialized.clone(),
            ..Transform3D::default()
        }, //
    ];

    let all_arch = [
        Transform3D::clear_fields(),
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

    for (expected, arch) in all_expected.into_iter().zip(all_arch) {
        similar_asserts::assert_eq!(expected, arch);

        eprintln!("arch = {arch:#?}");
        let serialized = arch.to_arrow().unwrap();
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            eprintln!("{} = {array:#?}", field.name());
        }

        let deserialized = Transform3D::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}
