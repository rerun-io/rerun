use std::f32::consts::TAU;

use re_types::{
    archetypes::Asset3D,
    components::{Blob, MediaType, OutOfTreeTransform3D},
    datatypes::{
        Angle, Rotation3D, RotationAxisAngle, Scale3D, TranslationRotationScale3D, Utf8, Vec3D,
    },
    Archetype as _, AsComponents as _,
};

#[test]
fn roundtrip() {
    const BYTES: &[u8] = &[1, 2, 3, 4, 5, 6];

    let expected = Asset3D {
        blob: Blob(BYTES.to_vec().into()),
        media_type: Some(MediaType(Utf8(MediaType::GLTF.into()))),
        transform: Some(OutOfTreeTransform3D(
            re_types::datatypes::Transform3D::TranslationRotationScale(
                TranslationRotationScale3D {
                    translation: Some(Vec3D([1.0, 2.0, 3.0])),
                    rotation: Some(Rotation3D::AxisAngle(RotationAxisAngle {
                        axis: Vec3D([0.2, 0.2, 0.8]),
                        angle: Angle::Radians(0.5 * TAU),
                    })),
                    scale: Some(Scale3D::Uniform(42.0)),
                    from_parent: true,
                },
            ),
        )), //
    };

    let arch = Asset3D::from_file_contents(BYTES.to_vec(), Some(MediaType::gltf())).with_transform(
        re_types::datatypes::Transform3D::from_translation_rotation_scale(
            [1.0, 2.0, 3.0],
            RotationAxisAngle::new([0.2, 0.2, 0.8], Angle::Radians(0.5 * TAU)),
            42.0,
        )
        .from_parent(),
    );
    similar_asserts::assert_eq!(expected, arch);

    // let expected_extensions: HashMap<_, _> = [
    // ]
    // .into();

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name);

        // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
        // has been fully replaced.
        // util::assert_extensions(
        //     &**array,
        //     expected_extensions[field.name.as_str()].as_slice(),
        // );
    }

    let deserialized = Asset3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

mod util;
