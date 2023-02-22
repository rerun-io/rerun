use std::{num::NonZeroU64, ops::Range};

use ecolor::Rgba;
use smallvec::{smallvec, SmallVec};

use crate::{
    context::uniform_buffer_allocation_size,
    debug_label::DebugLabel,
    resource_managers::{GpuTexture2DHandle, ResourceManagerError, TextureManager2D},
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BufferDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuBuffer, WgpuResourcePools,
    },
};

/// Defines how mesh vertices are built.
///
/// Mesh vertices consist of two vertex buffers right now.
/// One for positions ([`glam::Vec3`]) and one for the rest, called [`mesh_vertices::MeshVertexData`] here
pub mod mesh_vertices {
    use crate::wgpu_resources::VertexBufferLayout;
    use smallvec::smallvec;

    /// Mesh vertex as used in gpu residing vertex buffers.
    #[repr(C, packed)]
    #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct MeshVertexData {
        pub normal: glam::Vec3, // TODO(andreas): Compress. Afaik Octahedral Mapping is the best by far, see https://jcgt.org/published/0003/02/01/
        pub texcoord: glam::Vec2,
        // TODO(andreas): More properties? Different kinds of vertices?
    }

    /// Vertex buffer layouts describing how vertex data should be layed out.
    ///
    /// Needs to be kept in sync with `mesh_vertex.wgsl`.
    pub fn vertex_buffer_layouts() -> [VertexBufferLayout; 2] {
        [
            VertexBufferLayout {
                array_stride: std::mem::size_of::<glam::Vec3>() as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: smallvec![
                    // Position
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 0,
                    },
                ],
            },
            VertexBufferLayout {
                array_stride: std::mem::size_of::<MeshVertexData>() as _,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: VertexBufferLayout::attributes_from_formats(
                    1,
                    [
                        wgpu::VertexFormat::Float32x3, // Normal
                        wgpu::VertexFormat::Float32x2, // Texcoord
                    ]
                    .into_iter(),
                ),
            },
        ]
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

    pub indices: Vec<u32>, // TODO(andreas): different index formats?
    pub vertex_positions: Vec<glam::Vec3>,
    pub vertex_data: Vec<mesh_vertices::MeshVertexData>,
    pub materials: SmallVec<[Material; 1]>,
}

#[derive(Clone)]
pub struct Material {
    pub label: DebugLabel,

    /// Index range within the owning [`Mesh`] that should be rendered with this material.
    pub index_range: Range<u32>,

    /// Base color texture, also known as albedo.
    /// (not optional, needs to be at least a 1pix texture with a color!)
    pub albedo: GpuTexture2DHandle,

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
    pub vertex_buffer_data_range: Range<u64>,

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
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct MaterialUniformBuffer {
        pub albedo_multiplier: wgpu_buffer_types::Vec4,
    }
}

impl GpuMesh {
    pub fn new(
        pools: &mut WgpuResourcePools,
        texture_manager: &TextureManager2D,
        mesh_bound_group_layout: GpuBindGroupLayoutHandle,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &Mesh,
    ) -> Result<Self, ResourceManagerError> {
        assert!(data.vertex_positions.len() == data.vertex_data.len());
        re_log::trace!(
            "uploading new mesh named {:?} with {} vertices and {} triangles",
            data.label.get(),
            data.vertex_positions.len(),
            data.indices.len() / 3
        );

        // TODO(andreas): Have a variant that gets this from a stack allocator.]
        // TODO(andreas): Don't use a queue to upload
        let vertex_buffer_positions_size =
            std::mem::size_of_val(data.vertex_positions.as_slice()) as u64;
        let vertex_buffer_data_size = std::mem::size_of_val(data.vertex_data.as_slice()) as u64;
        let vertex_buffer_combined_size = vertex_buffer_positions_size + vertex_buffer_data_size;

        let vertex_buffer_combined = {
            let vertex_buffer_combined = pools.buffers.alloc(
                device,
                &BufferDesc {
                    label: data.label.clone().push_str(" - vertices"),
                    size: vertex_buffer_combined_size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            );
            let mut staging_buffer = queue
                .write_buffer_with(
                    &vertex_buffer_combined,
                    0,
                    vertex_buffer_combined_size.try_into().unwrap(),
                )
                .unwrap(); // Fails only if mapping is bigger than buffer size.
            staging_buffer[..vertex_buffer_positions_size as usize]
                .copy_from_slice(bytemuck::cast_slice(&data.vertex_positions));
            staging_buffer
                [vertex_buffer_positions_size as usize..vertex_buffer_combined_size as usize]
                .copy_from_slice(bytemuck::cast_slice(&data.vertex_data));
            drop(staging_buffer);
            vertex_buffer_combined
        };

        let index_buffer_size = std::mem::size_of_val(data.indices.as_slice()) as u64;
        let index_buffer = {
            let index_buffer = pools.buffers.alloc(
                device,
                &BufferDesc {
                    label: data.label.clone().push_str(" - indices"),
                    size: index_buffer_size,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                },
            );
            let mut staging_buffer = queue
                .write_buffer_with(&index_buffer, 0, index_buffer_size.try_into().unwrap())
                .unwrap(); // Fails only if mapping is bigger than buffer size.
            staging_buffer[..index_buffer_size as usize]
                .copy_from_slice(bytemuck::cast_slice(&data.indices));
            drop(staging_buffer);
            index_buffer
        };

        let materials = {
            // Buffer for *all* materials
            let allocation_size_per_uniform_buffer =
                uniform_buffer_allocation_size::<gpu_data::MaterialUniformBuffer>(device);
            let combined_buffers_size =
                allocation_size_per_uniform_buffer * data.materials.len() as u64;
            let material_uniform_buffers = pools.buffers.alloc(
                device,
                &BufferDesc {
                    label: data.label.clone().push_str(" - material uniforms"),
                    size: combined_buffers_size,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
                    mapped_at_creation: false,
                },
            );

            let mut materials_staging_buffer = queue
                .write_buffer_with(
                    &material_uniform_buffers,
                    0,
                    NonZeroU64::new(combined_buffers_size).unwrap(),
                )
                .unwrap(); // Fails only if mapping is bigger than buffer size.

            let mut materials = SmallVec::with_capacity(data.materials.len());
            for (i, material) in data.materials.iter().enumerate() {
                // CAREFUL: Memory from `write_buffer_with` may not be aligned, causing bytemuck to fail at runtime if we use it to cast the memory to a slice!
                let material_buffer_range_start = i * allocation_size_per_uniform_buffer as usize;
                let material_buffer_range_end = material_buffer_range_start
                    + std::mem::size_of::<gpu_data::MaterialUniformBuffer>();

                materials_staging_buffer[material_buffer_range_start..material_buffer_range_end]
                    .copy_from_slice(bytemuck::bytes_of(&gpu_data::MaterialUniformBuffer {
                        albedo_multiplier: material.albedo_multiplier.into(),
                    }));

                let texture = texture_manager.get(&material.albedo)?;
                let bind_group = pools.bind_groups.alloc(
                    device,
                    &BindGroupDesc {
                        label: material.label.clone(),
                        entries: smallvec![
                            BindGroupEntry::DefaultTextureView(texture.handle),
                            BindGroupEntry::Buffer {
                                handle: material_uniform_buffers.handle,
                                offset: material_buffer_range_start as _,
                                size: NonZeroU64::new(std::mem::size_of::<
                                    gpu_data::MaterialUniformBuffer,
                                >() as u64)
                            }
                        ],
                        layout: mesh_bound_group_layout,
                    },
                    &pools.bind_group_layouts,
                    &pools.textures,
                    &pools.buffers,
                    &pools.samplers,
                );

                materials.push(GpuMaterial {
                    index_range: material.index_range.clone(),
                    bind_group,
                });
            }
            materials
        };

        Ok(GpuMesh {
            index_buffer,
            vertex_buffer_combined,
            vertex_buffer_positions_range: 0..vertex_buffer_positions_size,
            vertex_buffer_data_range: vertex_buffer_positions_size..vertex_buffer_combined_size,
            index_buffer_range: 0..index_buffer_size,
            materials,
        })
    }
}
