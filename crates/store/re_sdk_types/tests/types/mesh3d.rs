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
        face_rendering: None,
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

#[test]
fn ply_parses_mesh_with_normals_colors_and_triangulates_faces() {
    let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property float z
property float nx
property float ny
property float nz
property uchar red
property uchar green
property uchar blue
property float temperature
element face 2
property list uchar int vertex_indices
property uchar material_index
end_header
0 0 0 0 0 1 255 0 0 10
1 0 0 0 0 1 0 255 0 11
1 1 0 0 0 1 0 0 255 12
0 1 0 0 0 1 255 255 0 13
3 0 1 2 7
4 0 2 3 1 8
"#;

    let parsed = Mesh3D::from_file_contents(contents).unwrap();
    let expected = Mesh3D::new([
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ])
    .with_vertex_normals([[0.0, 0.0, 1.0]; 4])
    .with_vertex_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF])
    .with_triangle_indices([[0, 1, 2], [0, 2, 3], [0, 3, 1]]);

    similar_asserts::assert_eq!(parsed, expected);
}

#[test]
fn ply_parses_xy_mesh_as_z0_mesh3d() {
    let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property uchar red
property uchar green
property uchar blue
element face 1
property list uchar int vertex_indices
end_header
0 0 255 0 0
1 0 0 255 0
1 1 0 0 255
0 1 255 255 0
4 0 1 2 3
"#;

    let parsed = Mesh3D::from_file_contents(contents).unwrap();
    let expected = Mesh3D::new([
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ])
    .with_vertex_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF])
    .with_triangle_indices([[0, 1, 2], [0, 2, 3]]);

    similar_asserts::assert_eq!(parsed, expected);
}

#[test]
fn ply_parses_mesh_with_vertex_index_alias() {
    let contents = br#"ply
format ascii 1.0
element vertex 3
property float x
property float y
property float z
element face 1
property list uchar int vertex_index
end_header
0 0 0
1 0 0
0 1 0
3 0 1 2
"#;

    let parsed = Mesh3D::from_file_contents(contents).unwrap();
    let expected = Mesh3D::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
        .with_triangle_indices([[0, 1, 2]]);

    similar_asserts::assert_eq!(parsed, expected);
}

#[test]
fn ply_prefers_vertex_indices_over_vertex_index_when_both_are_present() {
    let contents = br#"ply
format ascii 1.0
element vertex 3
property float x
property float y
property float z
element face 1
property list uchar float vertex_index
property list uchar int vertex_indices
end_header
0 0 0
1 0 0
0 1 0
3 9 8 7 3 0 1 2
"#;

    let parsed = Mesh3D::from_file_contents(contents).unwrap();
    let expected = Mesh3D::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
        .with_triangle_indices([[0, 1, 2]]);

    similar_asserts::assert_eq!(parsed, expected);
}

#[test]
fn ply_rejects_zero_face_mesh() {
    let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property uchar red
property uchar green
property uchar blue
element face 0
property list uchar int vertex_indices
end_header
0 0 255 0 0
1 0 0 255 0
1 1 0 0 255
0 1 255 255 0
"#;

    let err = Mesh3D::from_file_contents(contents).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string()
            .contains("at least one face with 3 or more vertex indices")
    );
}

#[test]
fn ply_rejects_zero_face_mesh_without_face_indices() {
    let contents = br#"ply
format ascii 1.0
element vertex 4
property float x
property float y
property uchar red
property uchar green
property uchar blue
element face 0
property int material_index
end_header
0 0 255 0 0
1 0 0 255 0
1 1 0 0 255
0 1 255 255 0
"#;

    let err = Mesh3D::from_file_contents(contents).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string()
            .contains("require \"vertex_indices\" or \"vertex_index\"")
    );
}

#[test]
fn ply_rejects_supported_face_properties_with_unsupported_types() {
    let contents = br#"ply
format ascii 1.0
element vertex 3
property float x
property float y
property float z
element face 1
property list uchar float vertex_indices
end_header
0 0 0
1 0 0
0 1 0
3 0 1 2
"#;

    let err = Mesh3D::from_file_contents(contents).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("PLY property 'vertex_indices'"));
}

#[test]
fn ply_rejects_missing_face_element() {
    let contents = br#"ply
format ascii 1.0
element vertex 3
property float x
property float y
property float z
end_header
0 0 0
1 0 0
0 1 0
"#;

    let err = Mesh3D::from_file_contents(contents).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}
