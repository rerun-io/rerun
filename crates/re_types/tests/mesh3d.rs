use std::collections::HashMap;

use re_types::{
    archetypes::Mesh3D,
    components::{ClassId, Position3D, Texcoord2D, Vector3D},
    datatypes::{
        Material, MeshProperties, Rgba32, TensorBuffer, TensorData, TensorDimension, Vec2D, Vec3D,
    },
    Archetype as _, AsComponents as _,
};

#[test]
fn roundtrip() {
    let tensor_data: re_types::components::TensorData = TensorData {
        shape: vec![
            TensorDimension {
                size: 2,
                name: Some("height".into()),
            },
            TensorDimension {
                size: 3,
                name: Some("width".into()),
            },
        ],
        buffer: TensorBuffer::U8(vec![1, 2, 3, 4, 5, 6].into()),
    }
    .into();

    let expected = Mesh3D {
        vertex_positions: vec![
            Position3D(Vec3D([1.0, 2.0, 3.0])),
            Position3D(Vec3D([10.0, 20.0, 30.0])),
        ],
        mesh_properties: Some(
            MeshProperties {
                indices: Some([1, 2, 3, 4, 5, 6].to_vec().into()),
            }
            .into(),
        ),
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
        mesh_material: Some(
            Material {
                albedo_factor: Some(Rgba32::from_unmultiplied_rgba(0xEE, 0x11, 0x22, 0x33)),
            }
            .into(),
        ),
        albedo_texture: Some(tensor_data.clone()),
        class_ids: Some(vec![
            ClassId::from(126), //
            ClassId::from(127), //
        ]),
    };

    let arch = Mesh3D::new([[1.0, 2.0, 3.0], [10.0, 20.0, 30.0]])
        .with_mesh_properties(MeshProperties::from_triangle_indices([
            (1, 2, 3),
            (4, 5, 6),
        ]))
        .with_vertex_normals([[4.0, 5.0, 6.0], [40.0, 50.0, 60.0]])
        .with_vertex_colors([0xAA0000CC, 0x00BB00DD])
        .with_vertex_texcoords([[0.0, 1.0], [2.0, 3.0]])
        .with_mesh_material(Material::from_albedo_factor(0xEE112233))
        .with_class_ids([126, 127])
        .with_albedo_texture(tensor_data);
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

    let deserialized = Mesh3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

mod util;
