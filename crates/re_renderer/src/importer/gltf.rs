use ahash::{HashMap, HashMapExt};
use anyhow::Context as _;
use smallvec::SmallVec;

use crate::{
    mesh::{mesh_vertices::MeshVertexData, Material, Mesh},
    renderer::MeshInstance,
    resource_managers::{MeshHandle, MeshManager, ResourceLifeTime, TextureManager2D},
};

use super::to_uniform_scale;

/// Loads both gltf and glb.
pub fn load_gltf_from_buffer(
    buffer: &[u8],
    lifetime: ResourceLifeTime,
    mesh_manager: &mut MeshManager,
    texture_manager: &mut TextureManager2D,
) -> anyhow::Result<Vec<MeshInstance>> {
    let (doc, buffers, _images) = gltf::import_slice(buffer)?;

    // let mut materials = HashMap::with_capacity(doc.materials().len());
    // for ref material in doc.materials() {
    //     // TODO(andreas): material manager?
    //     // TODO(andreas): grab actual texture
    //     let albedo = texture_manager.placeholder_texture();
    //     materials.insert(
    //         material.index(),
    //         Material {
    //             label: material.name().into(),
    //             // no material manager (yet?), so copy materials and fill out index range as we go.
    //             index_range: 0..0,
    //             albedo,
    //         },
    //     );
    // }

    let mut meshes = HashMap::with_capacity(doc.meshes().len());
    for ref mesh in doc.meshes() {
        let re_mesh = import_mesh(mesh, &buffers, texture_manager) //&materials)
            .with_context(|| format!("mesh {} (name {:?})", mesh.index(), mesh.name()))?;
        meshes.insert(mesh.index(), mesh_manager.store_resource(re_mesh, lifetime));
    }

    let mut instances = Vec::new();
    for scene in doc.scenes() {
        for node in scene.nodes() {
            gather_instances_recursive(
                &mut instances,
                &node,
                &macaw::Conformal3::IDENTITY,
                &meshes,
            );
        }
    }

    Ok(instances)
}

fn import_mesh(
    mesh: &gltf::Mesh<'_>,
    buffers: &[gltf::buffer::Data],
    texture_manager: &mut TextureManager2D, //imported_materials: HashMap<usize, Material>,
) -> anyhow::Result<Mesh> {
    let mut indices = Vec::new();
    let mut vertex_positions = Vec::new();
    let mut vertex_data = Vec::new();
    let mut materials = SmallVec::new();

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

        let index_offset = indices.len() as u32;
        if let Some(primitive_indices) = reader.read_indices() {
            indices.extend(primitive_indices.into_u32());
        } else {
            anyhow::bail!("Gltf primitives must have indices");
        }

        if vertex_positions.len() != vertex_data.len() {
            anyhow::bail!("Number of positions was not equal number of other vertex data.");
        }

        // TODO(andreas): Actually import stuff.
        let albedo = texture_manager.placeholder_texture();
        materials.push(Material {
            label: primitive.material().name().into(),
            index_range: index_offset..indices.len() as u32,
            albedo,
        })
    }
    if vertex_positions.is_empty() || indices.is_empty() {
        anyhow::bail!("empty mesh");
    }

    Ok(Mesh {
        label: mesh.name().into(),
        indices,
        vertex_positions,
        vertex_data,
        materials,
    })
}

fn gather_instances_recursive(
    instances: &mut Vec<MeshInstance>,
    node: &gltf::Node<'_>,
    transform: &macaw::Conformal3,
    meshes: &HashMap<usize, MeshHandle>,
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
        gather_instances_recursive(instances, &child, &transform, meshes);
    }

    if let Some(mesh) = node.mesh() {
        if let Some(mesh) = meshes.get(&mesh.index()) {
            instances.push(MeshInstance {
                mesh: mesh.clone(),
                world_from_mesh: transform,
            });
        }
    }
}
