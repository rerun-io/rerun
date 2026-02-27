use re_sdk_types::archetypes::Mesh3D;
use re_sdk_types::components::{
    AlbedoFactor, ClassId, Color, Position3D, Texcoord2D, TriangleIndices, Vector3D,
};
use re_sdk_types::datatypes::{Rgba32, UVec3D, Vec2D, Vec3D};
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _};

#[test]
fn roundtrip() {
    let texture_format = re_sdk_types::components::ImageFormat::rgb8([2, 3]);
    let texture_buffer = re_sdk_types::components::ImageBuffer::from(vec![0x42_u8; 2 * 3 * 3]);

    let expected = Mesh3D {
        vertex_positions: vec![
            Position3D(Vec3D([1.0, 2.0, 3.0])),
            Position3D(Vec3D([10.0, 20.0, 30.0])),
        ]
        .serialized(Mesh3D::descriptor_vertex_positions()),
        triangle_indices: vec![
            TriangleIndices(UVec3D([1, 2, 3])), //
            TriangleIndices(UVec3D([4, 5, 6])), //
        ]
        .serialized(Mesh3D::descriptor_triangle_indices()),
        vertex_normals: vec![
            Vector3D(Vec3D([4.0, 5.0, 6.0])),    //
            Vector3D(Vec3D([40.0, 50.0, 60.0])), //
        ]
        .serialized(Mesh3D::descriptor_vertex_normals()),
        vertex_colors: vec![
            Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC),
            Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]
        .serialized(Mesh3D::descriptor_vertex_colors()),
        vertex_texcoords: vec![
            Texcoord2D(Vec2D([0.0, 1.0])), //
            Texcoord2D(Vec2D([2.0, 3.0])), //
        ]
        .serialized(Mesh3D::descriptor_vertex_texcoords()),
        albedo_factor: AlbedoFactor(Rgba32::from_unmultiplied_rgba(0xEE, 0x11, 0x22, 0x33))
            .serialized(Mesh3D::descriptor_albedo_factor()),
        albedo_texture_format: texture_format
            .serialized(Mesh3D::descriptor_albedo_texture_format()),
        albedo_texture_buffer: texture_buffer
            .serialized(Mesh3D::descriptor_albedo_texture_buffer()),
        class_ids: vec![
            ClassId::from(126), //
            ClassId::from(127), //
        ]
        .serialized(Mesh3D::descriptor_class_ids()),
        shader_source: None,
        shader_parameters: None,
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

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = Mesh3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}
