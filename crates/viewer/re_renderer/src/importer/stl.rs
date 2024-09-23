use std::sync::Arc;

use itertools::Itertools;
use smallvec::smallvec;
use tinystl::StlData;

use crate::{
    mesh::{self, GpuMesh},
    renderer::MeshInstance,
    RenderContext, Rgba32Unmul,
};
use re_types::{archetypes::Asset3D, components::Color};

#[derive(thiserror::Error, Debug)]
pub enum StlImportError {
    #[error("Error loading STL mesh: {0}")]
    TinyStl(tinystl::Error),

    #[error(transparent)]
    MeshError(#[from] mesh::MeshError),
}

/// Load a [STL .stl file](https://en.wikipedia.org/wiki/STL_(file_format)) into the mesh manager.
pub fn load_stl_from_buffer(
    asset3d: &Asset3D,
    ctx: &RenderContext,
    _texture_key: u64,
) -> Result<Vec<MeshInstance>, StlImportError> {
    re_tracing::profile_function!();

    let Asset3D {
        blob,
        vertex_colors,
        albedo_factor,
        ..
    } = asset3d;

    let buffer = blob.as_slice();

    let cursor = std::io::Cursor::new(buffer);
    let StlData {
        name,
        triangles,
        normals,
        ..
    } = StlData::read_buffer(std::io::BufReader::new(cursor)).map_err(StlImportError::TinyStl)?;

    let num_vertices = triangles.len() * 3;
    let vertex_positions: &[glam::Vec3] = bytemuck::cast_slice(&triangles);
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

    let material = mesh::Material {
        label: name.clone().into(),
        index_range: 0..num_vertices as u32,
        albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
        albedo_factor: albedo_factor.map_or(crate::Rgba::WHITE, |c| c.0.into()),
    };

    let mesh = mesh::Mesh {
        label: name.into(),
        triangle_indices: (0..num_vertices as u32)
            .tuples::<(_, _, _)>()
            .map(glam::UVec3::from)
            .collect::<Vec<_>>(),
        vertex_positions: bytemuck::cast_slice(&triangles).to_vec(),

        // Normals on STL are per triangle, not per vertex.
        // Yes, this makes STL always look faceted.
        vertex_normals: normals
            .into_iter()
            .flat_map(|n| {
                let n = glam::Vec3::from_array(n);
                [n, n, n]
            })
            .collect(),

        vertex_colors,
        // STL has no texcoords.
        vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],

        materials: smallvec![material],
    };

    mesh.sanity_check()?;

    Ok(vec![MeshInstance::new_with_cpu_mesh(
        Arc::new(GpuMesh::new(ctx, &mesh)?),
        Some(Arc::new(mesh)),
    )])
}

pub fn clamped_vec_or_empty_color(values: &[Color], clamped_len: usize) -> Vec<Color> {
    if values.len() == clamped_len {
        // Happy path
        values.to_vec() // TODO(emilk): return a slice reference instead, in a `Cow` or similar
    } else if let Some(last) = values.last() {
        if values.len() == 1 {
            // Commo happy path
            return vec![*last; clamped_len];
        } else if values.len() < clamped_len {
            // Clamp
            let mut vec = Vec::with_capacity(clamped_len);
            vec.extend(values.iter());
            vec.extend(std::iter::repeat(last).take(clamped_len - values.len()));
            vec
        } else {
            // Trim
            values.iter().take(clamped_len).copied().collect()
        }
    } else {
        // Empty input
        Vec::new()
    }
}
