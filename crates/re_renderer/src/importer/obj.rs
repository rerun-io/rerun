use anyhow::Context;

use crate::mesh::{mesh_vertices::MeshVertexData, MeshData};

pub fn load_obj_from_buffer(buffer: &[u8]) -> anyhow::Result<Vec<MeshData>> {
    let (models, _materials) = tobj::load_obj_buf(
        &mut std::io::Cursor::new(buffer),
        &tobj::LoadOptions {
            single_index: true,
            triangulate: true,
            ..Default::default()
        },
        |_material_path| Err(tobj::LoadError::MaterialParseError),
    )
    .context("failed loading obj")?;

    Ok(models
        .iter()
        .map(|mesh| {
            let mesh = &mesh.mesh;
            let vertex_positions = mesh
                .positions
                .chunks(3)
                .map(|p| glam::vec3(p[0], p[1], p[2]))
                .collect();
            let vertex_data = mesh
                .normals
                .chunks(3)
                .zip(mesh.texcoords.chunks(2))
                .map(|(n, t)| MeshVertexData {
                    normal: glam::vec3(n[0], n[1], n[2]),
                    texcoord: glam::vec2(t[0], t[1]),
                })
                .collect();

            MeshData {
                label: "rerun logo".into(),
                indices: mesh.indices.clone(),
                vertex_positions,
                vertex_data,
            }
        })
        .collect())
}
