use std::collections::HashMap;

use re_types::{
    archetypes::Mesh3D,
    components::{ClassId, Position3D, Texcoord2D, TriangleIndices, Vector3D},
    datatypes::{Rgba32, UVec3D, Vec2D, Vec3D},
    Archetype as _, AsComponents as _,
};

use crate::util;

#[test]
fn roundtrip() {
    let texture_format = re_types::components::ImageFormat::rgb8([2, 3]);
    let texture_buffer = re_types::components::ImageBuffer::from(vec![0x42_u8; 2 * 3 * 3]);

    let expected = Mesh3D {
        vertex_positions: vec![
            Position3D(Vec3D([1.0, 2.0, 3.0])),
            Position3D(Vec3D([10.0, 20.0, 30.0])),
        ],
        triangle_indices: Some(vec![
            TriangleIndices(UVec3D([1, 2, 3])), //
            TriangleIndices(UVec3D([4, 5, 6])), //
        ]),
        vertex_normals: Some(vec![
            Vector3D(Vec3D([4.0, 5.0, 6.0])),    //
            Vector3D(Vec3D([40.0, 50.0, 60.0])), //
        ]),
        vertex_colors: Some(vec![
            Rgba32::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC).into(), //
            Rgba32::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD).into(),
        ]),
        vertex_texcoords: Some(vec![
            Texcoord2D(Vec2D([0.0, 1.0])), //
            Texcoord2D(Vec2D([2.0, 3.0])), //
        ]),
        albedo_factor: Some(Rgba32::from_unmultiplied_rgba(0xEE, 0x11, 0x22, 0x33).into()),
        albedo_texture_format: Some(texture_format),
        albedo_texture_buffer: Some(texture_buffer.clone()),
        class_ids: Some(vec![
            ClassId::from(126), //
            ClassId::from(127), //
        ]),
    };

    let arch = Mesh3D::new([[1.0, 2.0, 3.0], [10.0, 20.0, 30.0]])
        .with_triangle_indices([[1, 2, 3], [4, 5, 6]])
        .with_vertex_normals([[4.0, 5.0, 6.0], [40.0, 50.0, 60.0]])
        .with_vertex_colors([0xAA0000CC, 0x00BB00DD])
        .with_vertex_texcoords([[0.0, 1.0], [2.0, 3.0]])
        .with_albedo_factor(0xEE112233)
        .with_class_ids([126, 127])
        .with_albedo_texture(texture_format, texture_buffer);
    similar_asserts::assert_eq!(expected, arch);

    let expected_extensions: HashMap<_, _> = [
        ("class_ids", vec!["rerun.components.ClassId"]), //
    ]
    .into();

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

    let deserialized = Mesh3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}
