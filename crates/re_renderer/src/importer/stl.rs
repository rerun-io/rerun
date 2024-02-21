use itertools::Itertools;
use smallvec::smallvec;
use tinystl::StlData;

use crate::{mesh, renderer::MeshInstance, resource_managers::ResourceLifeTime, RenderContext};

#[derive(thiserror::Error, Debug)]
pub enum StlImportError {
    #[error("Error loading STL mesh: {0}")]
    TinyStl(tinystl::Error),

    #[error(transparent)]
    MeshError(#[from] mesh::MeshError),

    #[error(transparent)]
    ResourceManagerError(#[from] crate::resource_managers::ResourceManagerError),
}

/// Load a [STL .stl file](https://en.wikipedia.org/wiki/STL_(file_format)) into the mesh manager.
pub fn load_stl_from_buffer(
    buffer: &[u8],
    ctx: &RenderContext,
) -> Result<Vec<MeshInstance>, StlImportError> {
    re_tracing::profile_function!();

    let cursor = std::io::Cursor::new(buffer);
    let StlData {
        name,
        triangles,
        normals,
        ..
    } = StlData::read_buffer(std::io::BufReader::new(cursor)).map_err(StlImportError::TinyStl)?;

    let num_vertices = triangles.len() * 3;

    let material = mesh::Material {
        label: "default material".into(),
        index_range: 0..num_vertices as u32,
        albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
        albedo_multiplier: crate::Rgba::WHITE,
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

        // STL has neither colors nor texcoords.
        vertex_colors: vec![crate::Rgba32Unmul::WHITE; num_vertices],
        vertex_texcoords: vec![glam::Vec2::ZERO; num_vertices],

        materials: smallvec![material],
    };

    mesh.sanity_check()?;

    let gpu_mesh = ctx
        .mesh_manager
        .write()
        .create(ctx, &mesh, ResourceLifeTime::LongLived)?;

    Ok(vec![MeshInstance {
        gpu_mesh,
        mesh: Some(std::sync::Arc::new(mesh)),
        ..Default::default()
    }])
}
