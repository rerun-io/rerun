use arrow::array::{self, Array as _};

use super::Mesh3D;
use crate::archetypes;
use crate::components;

#[derive(thiserror::Error, Debug)]
pub enum Mesh3DError {
    #[error(
        "No indices were specified, so the number of positions must be divisible by 9 [(xyz xyz xyz), …], got {0}"
    )]
    PositionsAreNotTriangles(usize),

    #[error("Index out of bounds: got index={index} with {num_vertices} vertices")]
    IndexOutOfBounds { index: u32, num_vertices: usize },

    #[error(
        "Positions & normals array must have the same length, \
        got positions={0} vs. normals={1}"
    )]
    MismatchedPositionsNormals(usize, usize),
}

impl Mesh3D {
    /// Use this image as the albedo texture.
    #[inline]
    pub fn with_albedo_texture_image(mut self, image: impl Into<archetypes::Image>) -> Self {
        let image = image.into();

        self.albedo_texture_format = image
            .format
            .map(|batch| batch.with_descriptor_override(Self::descriptor_albedo_texture_format()));
        self.albedo_texture_buffer = image
            .buffer
            .map(|batch| batch.with_descriptor_override(Self::descriptor_albedo_texture_buffer()));
        self
    }

    /// Use this image as the albedo texture.
    #[inline]
    pub fn with_albedo_texture(
        self,
        image_format: impl Into<components::ImageFormat>,
        image_buffer: impl Into<components::ImageBuffer>,
    ) -> Self {
        self.with_albedo_texture_format(image_format)
            .with_albedo_texture_buffer(image_buffer)
    }

    /// Check that this is a valid mesh, e.g. that the vertex indices are within bounds
    /// and that we have the same number of positions and normals (if any).
    ///
    /// Only use this when logging a whole new mesh. Not meaningful for field updates!
    #[track_caller]
    pub fn sanity_check(&self) -> Result<(), Mesh3DError> {
        let num_vertices = self.num_vertices();

        let index_data = self.triangle_indices.as_ref().map(|indices| {
            array::as_fixed_size_list_array(&indices.array)
                .values()
                .to_data()
        });

        if let Some(index_data) = index_data {
            for index in index_data.buffer::<u32>(0) {
                if num_vertices <= *index as usize {
                    return Err(Mesh3DError::IndexOutOfBounds {
                        index: *index,
                        num_vertices,
                    });
                }
            }
        } else if !num_vertices.is_multiple_of(9) {
            return Err(Mesh3DError::PositionsAreNotTriangles(num_vertices));
        }

        if let Some(normals) = &self.vertex_normals
            && normals.array.len() != num_vertices
        {
            return Err(Mesh3DError::MismatchedPositionsNormals(
                num_vertices,
                normals.array.len(),
            ));
        }

        Ok(())
    }

    /// The total number of vertices.
    #[inline]
    pub fn num_vertices(&self) -> usize {
        self.vertex_positions
            .as_ref()
            .map_or(0, |positions| positions.array.len())
    }

    /// The total number of triangles.
    #[inline]
    pub fn num_triangles(&self) -> usize {
        if let Some(triangle_indices) = self.triangle_indices.as_ref() {
            triangle_indices.array.len()
        } else {
            self.num_vertices() / 3
        }
    }

    /// Attach a custom WGSL fragment shader with optional parameter metadata.
    ///
    /// The `wgsl_source` replaces the default Phong lighting fragment shader.
    /// The shader's fragment entry point must be named `fs_main`.
    ///
    /// `parameters_json` is an optional JSON string describing uniform parameters,
    /// their types, and source entity paths for data binding.
    #[inline]
    pub fn with_shader(
        self,
        wgsl_source: impl Into<components::ShaderSource>,
        parameters_json: Option<impl Into<components::ShaderParameters>>,
    ) -> Self {
        let result = self.with_shader_source(wgsl_source);
        if let Some(params) = parameters_json {
            result.with_shader_parameters(params)
        } else {
            result
        }
    }
}
