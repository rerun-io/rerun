use itertools::Itertools as _;
use smallvec::smallvec;

use crate::{CpuModel, DebugLabel, RenderContext, mesh};

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

    // TODO(hmeyer/stl_io#26): use optional name from ascii stl files.
    // https://github.com/hmeyer/stl_io/pull/26
    let name = DebugLabel::from("");

    let (normals, triangles): (Vec<_>, Vec<_>) = reader
        .into_iter()
        .map(|triangle_res| {
            triangle_res.map(|triangle| {
                (
                    [triangle.normal.0, triangle.normal.0, triangle.normal.0],
                    [
                        triangle.vertices[0].0,
                        triangle.vertices[1].0,
                        triangle.vertices[2].0,
                    ],
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(StlImportError::StlIoError)?
        .into_iter()
        .unzip();

    let num_vertices = triangles.len() * 3;

    let material = mesh::Material {
        label: name.clone(),
        index_range: 0..num_vertices as u32,
        albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
        albedo_factor: crate::Rgba::WHITE,
    };

    let vertex_positions = bytemuck::cast_vec(triangles);
    let bbox = macaw::BoundingBox::from_points(vertex_positions.iter().copied());

    let mesh = mesh::CpuMesh {
        label: name.clone(),
        triangle_indices: (0..num_vertices as u32)
            .tuples::<(_, _, _)>()
            .map(glam::UVec3::from)
            .collect::<Vec<_>>(),

        vertex_positions,

        // Normals on STL are per triangle, not per vertex.
        // Yes, this makes STL always look faceted.
        vertex_normals: bytemuck::cast_vec(normals),

        // STL has neither colors nor texcoords.
        vertex_colors: vec![crate::Rgba32Unmul::WHITE; num_vertices],
        vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],

        materials: smallvec![material],

        bbox,
    };

    mesh.sanity_check()?;

    Ok(CpuModel::from_single_mesh(mesh))
}
