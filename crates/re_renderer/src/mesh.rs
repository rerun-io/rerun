use std::{mem::size_of, ops::Range};

use ecolor::Rgba;
use smallvec::{smallvec, SmallVec};

use crate::{
    allocator::create_and_fill_uniform_buffer_batch,
    debug_label::DebugLabel,
    resource_managers::{GpuTexture2D, ResourceManagerError},
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BufferDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuBuffer,
    },
    RenderContext, Rgba32Unmul,
};

/// Defines how mesh vertices are built.
pub mod mesh_vertices {
    use crate::wgpu_resources::VertexBufferLayout;

    /// Vertex buffer layouts describing how vertex data should be laid out.
    ///
    /// Needs to be kept in sync with `mesh_vertex.wgsl`.
    pub fn vertex_buffer_layouts() -> smallvec::SmallVec<[VertexBufferLayout; 4]> {
        // TODO(andreas): Compress normals. Afaik Octahedral Mapping is the best by far, see https://jcgt.org/published/0003/02/01/
        VertexBufferLayout::from_formats(
            [
                wgpu::VertexFormat::Float32x3, // position
                wgpu::VertexFormat::Unorm8x4,  // RGBA
                wgpu::VertexFormat::Float32x3, // normal
                wgpu::VertexFormat::Float32x2, // texcoord
            ]
            .into_iter(),
        )
    }

    /// Next vertex attribute index that can be used for another vertex buffer.
    pub fn next_free_shader_location() -> u32 {
        vertex_buffer_layouts()
            .iter()
            .flat_map(|layout| layout.attributes.iter())
            .max_by(|a1, a2| a1.shader_location.cmp(&a2.shader_location))
            .unwrap()
            .shader_location
            + 1
    }
}

#[derive(Clone)]
pub struct Mesh {
    pub label: DebugLabel,

    pub triangle_indices: Vec<glam::UVec3>,

    pub vertex_positions: Vec<glam::Vec3>,

    /// Per-vertex albedo color.
    /// Must be equal in length to [`Self::vertex_positions`].
    pub vertex_colors: Vec<Rgba32Unmul>,

    /// Must be equal in length to [`Self::vertex_positions`].
    /// Use ZERO for unshaded.
    pub vertex_normals: Vec<glam::Vec3>,

    /// Must be equal in length to [`Self::vertex_positions`].
    pub vertex_texcoords: Vec<glam::Vec2>,

    pub materials: SmallVec<[Material; 1]>,
}

impl Mesh {
    pub fn sanity_check(&self) -> Result<(), MeshError> {
        re_tracing::profile_function!();

        let num_pos = self.vertex_positions.len();
        let num_color = self.vertex_colors.len();
        let num_normals = self.vertex_normals.len();
        let num_texcoords = self.vertex_texcoords.len();

        if num_pos != num_color {
            return Err(MeshError::WrongNumberOfColors { num_pos, num_color });
        }
        if num_pos != num_normals {
            return Err(MeshError::WrongNumberOfNormals {
                num_pos,
                num_normals,
            });
        }
        if num_pos != num_texcoords {
            return Err(MeshError::WrongNumberOfTexcoord {
                num_pos,
                num_texcoords,
            });
        }

        for indices in &self.triangle_indices {
            let max_index = indices.max_element();
            if num_pos <= max_index as usize {
                return Err(MeshError::IndexOutOfBounds {
                    num_pos,
                    index: max_index,
                });
            }
        }

        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MeshError {
    #[error("Number of vertex positions {num_pos} differed from the number of vertex colors {num_color}")]
    WrongNumberOfColors { num_pos: usize, num_color: usize },

    #[error("Number of vertex positions {num_pos} differed from the number of vertex normals {num_normals}")]
    WrongNumberOfNormals { num_pos: usize, num_normals: usize },

    #[error("Number of vertex positions {num_pos} differed from the number of vertex tex-coords {num_texcoords}")]
    WrongNumberOfTexcoord {
        num_pos: usize,
        num_texcoords: usize,
    },

    #[error("Index {index} was out of bounds for {num_pos} vertex positions")]
    IndexOutOfBounds { num_pos: usize, index: u32 },
}

#[derive(Clone)]
pub struct Material {
    pub label: DebugLabel,

    /// Index range within the owning [`Mesh`] that should be rendered with this material.
    pub index_range: Range<u32>,

    /// Base color texture, also known as albedo.
    /// (not optional, needs to be at least a 1pix texture with a color!)
    pub albedo: GpuTexture2D,

    /// Factor applied to the decoded albedo color.
    pub albedo_multiplier: Rgba,
}

#[derive(Clone)]
pub(crate) struct GpuMesh {
    // It would be desirable to put both vertex and index buffer into the same buffer, BUT
    // WebGL doesn't allow us to do so! (see https://github.com/gfx-rs/wgpu/pull/3157)
    pub index_buffer: GpuBuffer,

    /// Buffer for all vertex data, subdivided in several sections for different vertex buffer bindings.
    /// See [`mesh_vertices`]
    pub vertex_buffer_combined: GpuBuffer,
    pub vertex_buffer_positions_range: Range<u64>,
    pub vertex_buffer_colors_range: Range<u64>,
    pub vertex_buffer_normals_range: Range<u64>,
    pub vertex_buffer_texcoord_range: Range<u64>,

    pub index_buffer_range: Range<u64>,

    /// Every mesh has at least one material.
    pub materials: SmallVec<[GpuMaterial; 1]>,
}

#[derive(Clone)]
pub(crate) struct GpuMaterial {
    /// Index range within the owning [`Mesh`] that should be rendered with this material.
    pub index_range: Range<u32>,

    pub bind_group: GpuBindGroup,
}

pub(crate) mod gpu_data {
    use crate::wgpu_buffer_types;

    /// Keep in sync with [`MaterialUniformBuffer`] in `instanced_mesh.wgsl`
    #[repr(C, align(256))]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct MaterialUniformBuffer {
        pub albedo_multiplier: wgpu_buffer_types::Vec4,
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 1],
    }
}

impl GpuMesh {
    // TODO(andreas): Take read-only context here and make uploads happen on staging belt.
    pub fn new(
        ctx: &RenderContext,
        mesh_bind_group_layout: GpuBindGroupLayoutHandle,
        data: &Mesh,
    ) -> Result<Self, ResourceManagerError> {
        data.sanity_check()?;

        re_log::trace!(
            "uploading new mesh named {:?} with {} vertices and {} triangles",
            data.label.get(),
            data.vertex_positions.len(),
            data.triangle_indices.len(),
        );

        // TODO(andreas): Have a variant that gets this from a stack allocator.
        let vb_positions_size = (data.vertex_positions.len() * size_of::<glam::Vec3>()) as u64;
        let vb_color_size = (data.vertex_colors.len() * size_of::<Rgba32Unmul>()) as u64;
        let vb_normals_size = (data.vertex_normals.len() * size_of::<glam::Vec3>()) as u64;
        let vb_texcoords_size = (data.vertex_texcoords.len() * size_of::<glam::Vec2>()) as u64;

        let vb_combined_size =
            vb_positions_size + vb_color_size + vb_normals_size + vb_texcoords_size;

        let pools = &ctx.gpu_resources;
        let device = &ctx.device;

        let vertex_buffer_combined = {
            let vertex_buffer_combined = pools.buffers.alloc(
                device,
                &BufferDesc {
                    label: format!("{} - vertices", data.label).into(),
                    size: vb_combined_size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            );

            let mut staging_buffer = ctx.cpu_write_gpu_read_belt.lock().allocate::<u8>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                vb_combined_size as _,
            )?;
            staging_buffer.extend_from_slice(bytemuck::cast_slice(&data.vertex_positions))?;
            staging_buffer.extend_from_slice(bytemuck::cast_slice(&data.vertex_colors))?;
            staging_buffer.extend_from_slice(bytemuck::cast_slice(&data.vertex_normals))?;
            staging_buffer.extend_from_slice(bytemuck::cast_slice(&data.vertex_texcoords))?;
            staging_buffer.copy_to_buffer(
                ctx.active_frame.before_view_builder_encoder.lock().get(),
                &vertex_buffer_combined,
                0,
            )?;
            vertex_buffer_combined
        };

        let index_buffer_size = (size_of::<glam::UVec3>() * data.triangle_indices.len()) as u64;
        let index_buffer = {
            let index_buffer = pools.buffers.alloc(
                device,
                &BufferDesc {
                    label: format!("{} - indices", data.label).into(),
                    size: index_buffer_size,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            );

            let mut staging_buffer = ctx.cpu_write_gpu_read_belt.lock().allocate::<glam::UVec3>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                data.triangle_indices.len(),
            )?;
            staging_buffer.extend_from_slice(bytemuck::cast_slice(&data.triangle_indices))?;
            staging_buffer.copy_to_buffer(
                ctx.active_frame.before_view_builder_encoder.lock().get(),
                &index_buffer,
                0,
            )?;
            index_buffer
        };

        let materials = {
            let uniform_buffer_bindings = create_and_fill_uniform_buffer_batch(
                ctx,
                format!("{} - material uniforms", data.label).into(),
                data.materials
                    .iter()
                    .map(|material| gpu_data::MaterialUniformBuffer {
                        albedo_multiplier: material.albedo_multiplier.into(),
                        end_padding: Default::default(),
                    }),
            );

            let mut materials = SmallVec::with_capacity(data.materials.len());
            for (material, uniform_buffer_binding) in data
                .materials
                .iter()
                .zip(uniform_buffer_bindings.into_iter())
            {
                let bind_group = pools.bind_groups.alloc(
                    device,
                    pools,
                    &BindGroupDesc {
                        label: material.label.clone(),
                        entries: smallvec![
                            BindGroupEntry::DefaultTextureView(material.albedo.handle()),
                            uniform_buffer_binding
                        ],
                        layout: mesh_bind_group_layout,
                    },
                );

                materials.push(GpuMaterial {
                    index_range: material.index_range.clone(),
                    bind_group,
                });
            }
            materials
        };

        let vb_colors_start = vb_positions_size;
        let vb_normals_start = vb_colors_start + vb_color_size;
        let vb_texcoord_start = vb_normals_start + vb_normals_size;

        Ok(GpuMesh {
            index_buffer,
            vertex_buffer_combined,
            vertex_buffer_positions_range: 0..vb_positions_size,
            vertex_buffer_colors_range: vb_colors_start..vb_normals_start,
            vertex_buffer_normals_range: vb_normals_start..vb_texcoord_start,
            vertex_buffer_texcoord_range: vb_texcoord_start..vb_combined_size,
            index_buffer_range: 0..index_buffer_size,
            materials,
        })
    }
}
