use itertools::Itertools as _;
use smallvec::smallvec;

use crate::{CpuModel, RenderContext, mesh};

#[derive(thiserror::Error, Debug)]
pub enum StlImportError {
    #[error("Error loading STL mesh: {0}")]
    StlIoError(std::io::Error),

    #[error(transparent)]
    MeshError(#[from] mesh::MeshError),
}

/// Load a [STL .stl file](https://en.wikipedia.org/wiki/STL_(file_format)) into the mesh manager.
pub fn load_stl_from_buffer(
    buffer: &[u8],
    ctx: &RenderContext,
) -> Result<CpuModel, StlImportError> {
    re_tracing::profile_function!();

    let mut cursor = std::io::Cursor::new(buffer);
    let reader = stl_io::create_stl_reader(&mut cursor).map_err(StlImportError::StlIoError)?;
    // TODO(hmeyer/stl_io#26): parse name from ASCII?
    // https://github.com/hmeyer/stl_io/pull/26
    let name = reader.name().cloned().unwrap_or_default();

    let (normals, vertices): (Vec<_>, Vec<_>) = reader
        .into_iter()
        .map(|triangle| triangle.unwrap())
        .map(|triangle| {
            (
                triangle.normal,
                [
                    triangle.vertices[0].0,
                    triangle.vertices[1].0,
                    triangle.vertices[2].0,
                ],
            )
        })
        .unzip();

    let num_vertices = vertices.len() * 3;

    let material = mesh::Material {
        label: name.clone().into(),
        index_range: 0..num_vertices as u32,
        albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
        albedo_factor: crate::Rgba::WHITE,
    };

    let mesh = mesh::CpuMesh {
        label: name.clone().into(),
        triangle_indices: (0..num_vertices as u32)
            .tuples::<(_, _, _)>()
            .map(glam::UVec3::from)
            .collect::<Vec<_>>(),

        vertex_positions: bytemuck::cast_vec(vertices),

        // Normals on STL are per triangle, not per vertex.
        // Yes, this makes STL always look faceted.
        vertex_normals: normals
            .iter()
            .flat_map(|n| {
                let n = glam::Vec3::from_array(n.0);
                [n, n, n]
            })
            .collect(),

        // STL has neither colors nor texcoords.
        vertex_colors: vec![crate::Rgba32Unmul::WHITE; num_vertices],
        vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],

        materials: smallvec![material],
    };

    mesh.sanity_check()?;

    Ok(CpuModel::from_single_mesh(mesh))
}
