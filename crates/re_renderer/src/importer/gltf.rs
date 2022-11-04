use ahash::HashMap;
use anyhow::Context as _;

use crate::mesh::{mesh_vertices::MeshVertexData, MeshData};

use super::{to_uniform_scale, ImportMeshInstance, ModelImportData};

/// Loads both gltf and glb.
pub fn load_gltf_from_buffer(buffer: &[u8]) -> anyhow::Result<ModelImportData> {
    let (doc, buffers, _images) = gltf::import_slice(buffer)?;

    let mut json_mesh_idx_to_local_idx = HashMap::default();
    let mut meshes = Vec::new();
    for ref mesh in doc.meshes() {
        let mesh_data = import_mesh(mesh, &buffers)
            .with_context(|| format!("mesh {} (name {:?})", mesh.index(), mesh.name()))?;

        json_mesh_idx_to_local_idx.insert(mesh.index(), meshes.len());
        meshes.push(mesh_data);
    }

    let mut instances = Vec::new();
    for scene in doc.scenes() {
        for node in scene.nodes() {
            gather_instances_recursive(
                &mut instances,
                &node,
                &macaw::Conformal3::IDENTITY,
                &json_mesh_idx_to_local_idx,
            );
        }
    }

    Ok(ModelImportData { meshes, instances })
}

fn import_mesh(mesh: &gltf::Mesh<'_>, buffers: &[gltf::buffer::Data]) -> anyhow::Result<MeshData> {
    let mut indices = Vec::new();
    let mut vertex_positions = Vec::new();
    let mut vertex_data = Vec::new();

    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| Some(&*buffers[buffer.index()]));

        if let Some(primitive_positions) = reader.read_positions() {
            vertex_positions.extend(primitive_positions.map(glam::Vec3::from));
        } else {
            anyhow::bail!("Gltf primitives must have positions");
        }
        if let Some(primitive_normals) = reader.read_normals() {
            // TODO(andreas): Texcoord (optional)
            // TODO(andreas): Generate normals if not present
            vertex_data.extend(primitive_normals.map(|p| MeshVertexData {
                normal: glam::Vec3::from(p),
                texcoord: glam::Vec2::ZERO,
            }));
        } else {
            anyhow::bail!("Gltf primitives must have normals");
        }
        if let Some(primitive_indices) = reader.read_indices() {
            indices.extend(primitive_indices.into_u32());
        } else {
            anyhow::bail!("Gltf primitives must have indices");
        }

        if vertex_positions.len() != vertex_data.len() {
            anyhow::bail!("Number of positions was not equal number of other vertex data.");
        }

        // TODO(andreas): Material
    }

    if vertex_positions.is_empty() || indices.is_empty() {
        anyhow::bail!("empty mesh");
    }

    Ok(MeshData {
        label: mesh.name().into(),
        indices,
        vertex_positions,
        vertex_data,
    })
}

fn gather_instances_recursive(
    instances: &mut Vec<ImportMeshInstance>,
    node: &gltf::Node<'_>,
    transform: &macaw::Conformal3,
    gltf_mesh_idx_to_local_idx: &HashMap<usize, usize>,
) {
    let (scale, rotation, translation) = match node.transform() {
        gltf::scene::Transform::Matrix { matrix } => {
            let matrix = glam::Mat4::from_cols_array_2d(&matrix);
            // gltf specifies there that matrices must be described by rotation, scale & translation only.
            matrix.to_scale_rotation_translation()
        }
        gltf::scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => (
            glam::Vec3::from(scale),
            glam::Quat::from_array(rotation),
            glam::Vec3::from(translation),
        ),
    };

    let node_transform = macaw::Conformal3::from_scale_rotation_translation(
        to_uniform_scale(scale),
        rotation,
        translation,
    );
    let transform = transform * node_transform;

    for child in node.children() {
        gather_instances_recursive(instances, &child, &transform, gltf_mesh_idx_to_local_idx);
    }

    if let Some(mesh) = node.mesh() {
        if let Some(mesh_idx) = gltf_mesh_idx_to_local_idx.get(&mesh.index()) {
            instances.push(ImportMeshInstance {
                mesh_idx: *mesh_idx,
                world_from_mesh: transform,
            });
        }
    }
}
