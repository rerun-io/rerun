use std::sync::Arc;

use re_types::components::{AlbedoFactor, Color};
use smallvec::smallvec;

use crate::{
    mesh::{GpuMesh, Material, Mesh, MeshError},
    renderer::MeshInstance,
    RenderContext, Rgba32Unmul,
};

use super::stl::clamped_vec_or_empty_color;

#[derive(thiserror::Error, Debug)]
pub enum ObjImportError {
    #[error(transparent)]
    ObjLoading(#[from] tobj::LoadError),

    #[error(transparent)]
    Mesh(#[from] MeshError),
}

/// Load a [Wavefront .obj file](https://en.wikipedia.org/wiki/Wavefront_.obj_file)
/// into the mesh & texture manager.
pub fn load_obj_from_buffer(
    buffer: &[u8],
    ctx: &RenderContext,
    vertex_colors: &Option<Vec<Color>>,
    albedo_factor: &Option<AlbedoFactor>,
) -> Result<Vec<MeshInstance>, ObjImportError> {
    re_tracing::profile_function!();

    let (models, _materials) = tobj::load_obj_buf(
        &mut std::io::Cursor::new(buffer),
        &tobj::LoadOptions {
            single_index: true,
            triangulate: true,
            ..Default::default()
        },
        |_material_path| Err(tobj::LoadError::MaterialParseError),
    )?;

    // TODO(andreas) Merge all obj meshes into a single re_renderer mesh with multiple materials.
    models
        .into_iter()
        .map(|model| {
            // This could be optimized by using bytemuck.

            let mesh = model.mesh;
            let vertex_positions: Vec<glam::Vec3> = mesh
                .positions
                .chunks_exact(3)
                .map(|p| glam::vec3(p[0], p[1], p[2]))
                .collect();

            let triangle_indices = mesh
                .indices
                .chunks_exact(3)
                .map(|p| glam::uvec3(p[0], p[1], p[2]))
                .collect();

            let num_positions = vertex_positions.len();

            let vertex_colors = if let Some(vertex_colors) = vertex_colors {
                let vertex_colors_arr =
                    clamped_vec_or_empty_color(vertex_colors.as_slice(), vertex_positions.len());
                re_tracing::profile_scope!("copy_colors");
                vertex_colors_arr
                    .iter()
                    .map(|c| Rgba32Unmul::from_rgba_unmul_array(c.to_array()))
                    .collect()
            } else {
                vec![Rgba32Unmul::WHITE; num_positions]
            };

            let mut vertex_normals: Vec<glam::Vec3> = mesh
                .normals
                .chunks_exact(3)
                .map(|n| glam::vec3(n[0], n[1], n[2]))
                .collect();
            vertex_normals.resize(vertex_positions.len(), glam::Vec3::ZERO);

            let mut vertex_texcoords: Vec<glam::Vec2> = mesh
                .texcoords
                .chunks_exact(2)
                .map(|t| glam::vec2(t[0], t[1]))
                .collect();
            vertex_texcoords.resize(vertex_positions.len(), glam::Vec2::ZERO);

            let texture = ctx.texture_manager_2d.white_texture_unorm_handle();

            let mesh = Mesh {
                label: model.name.into(),
                triangle_indices,
                vertex_positions,
                vertex_colors,
                vertex_normals,
                vertex_texcoords,

                // TODO(andreas): proper material loading
                materials: smallvec![Material {
                    label: "default material".into(),
                    index_range: 0..mesh.indices.len() as u32,
                    albedo: texture.clone(),
                    albedo_factor: albedo_factor.map_or(crate::Rgba::WHITE, |c| c.0.into()),
                }],
            };

            mesh.sanity_check()?;

            Ok(MeshInstance::new_with_cpu_mesh(
                Arc::new(GpuMesh::new(ctx, &mesh)?),
                Some(Arc::new(mesh)),
            ))
        })
        .collect()
}
