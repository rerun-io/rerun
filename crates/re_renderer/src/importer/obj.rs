use std::sync::Arc;

use anyhow::Context as _;
use smallvec::smallvec;

use crate::{
    mesh::{Material, Mesh},
    renderer::MeshInstance,
    resource_managers::ResourceLifeTime,
    RenderContext,
};

/// Load a [Wavefront .obj file](https://en.wikipedia.org/wiki/Wavefront_.obj_file)
/// into the mesh & texture manager.
pub fn load_obj_from_buffer(
    buffer: &[u8],
    lifetime: ResourceLifeTime,
    ctx: &mut RenderContext,
) -> anyhow::Result<Vec<MeshInstance>> {
    crate::profile_function!();

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

    // TODO(andreas) Merge all obj meshes into a single re_renderer mesh with multiple materials.
    Ok(models
        .into_iter()
        .map(|model| {
            // This could be optimized by using bytemuck.

            let mesh = model.mesh;
            let vertex_positions = mesh
                .positions
                .chunks_exact(3)
                .map(|p| glam::vec3(p[0], p[1], p[2]))
                .collect();

            let vertex_normals = mesh
                .normals
                .chunks_exact(3)
                .map(|n| glam::vec3(n[0], n[1], n[2]))
                .collect();

            let vertex_texcoords = mesh
                .texcoords
                .chunks_exact(2)
                .map(|t| glam::vec2(t[0], t[1]))
                .collect();

            let texture = ctx.texture_manager_2d.white_texture_handle();

            let num_indices = mesh.indices.len();

            let mesh = Mesh {
                label: model.name.into(),
                indices: mesh.indices,
                vertex_positions,
                vertex_normals,
                vertex_texcoords,

                // TODO(andreas): proper material loading
                materials: smallvec![Material {
                    label: "default material".into(),
                    index_range: 0..num_indices as u32,
                    albedo: texture.clone(),
                    albedo_multiplier: crate::Rgba::WHITE,
                }],
            };
            let gpu_mesh = ctx
                .mesh_manager
                .write()
                .create(ctx, &mesh, lifetime)
                .unwrap(); // TODO(andreas): Handle error
            MeshInstance {
                gpu_mesh,
                mesh: Some(Arc::new(mesh)),
                ..Default::default()
            }
        })
        .collect())
}
