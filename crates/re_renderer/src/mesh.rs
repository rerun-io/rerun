use std::ops::Range;

use smallvec::{smallvec, SmallVec};

use crate::{
    debug_label::DebugLabel,
    renderer::MeshRenderer,
    resource_managers::{ResourceManagerError, Texture2DHandle, TextureManager2D},
    wgpu_resource_pools::{
        bind_group_pool::{BindGroupDesc, BindGroupEntry, GpuBindGroupHandleStrong},
        buffer_pool::{BufferDesc, GpuBufferHandleStrong},
        WgpuResourcePools,
    },
};

/// Defines how mesh vertices are built.
///
/// Mesh vertices consist of two vertex buffers right now.
/// One for positions ([`glam::Vec3`]) and one for the rest, called [`mesh_vertices::MeshVertexData`] here
pub mod mesh_vertices {
    use crate::wgpu_resource_pools::render_pipeline_pool::VertexBufferLayout;
    use smallvec::smallvec;

    /// Mesh vertex as used in gpu residing vertex buffers.
    #[repr(C, packed)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
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
                attributes: smallvec![
                    // Normal
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x3,
                        offset: memoffset::offset_of!(MeshVertexData, normal) as _,
                        shader_location: 1,
                    },
                    // Texcoord
                    wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: memoffset::offset_of!(MeshVertexData, texcoord) as _,
                        shader_location: 2,
                    },
                ],
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
    pub albedo: Texture2DHandle,
}

#[derive(Clone)]
pub(crate) struct GpuMesh {
    // It would be desirable to put both vertex and index buffer into the same buffer, BUT
    // WebGL doesn't allow us to do so! (see https://github.com/gfx-rs/wgpu/pull/3157)
    pub index_buffer: GpuBufferHandleStrong,

    /// Buffer for all vertex data, subdivided in several sections for different vertex buffer bindings.
    /// See [`mesh_vertices`]
    pub vertex_buffer_combined: GpuBufferHandleStrong,
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

    pub bind_group: GpuBindGroupHandleStrong,
}

impl GpuMesh {
    pub fn new(
        pools: &mut WgpuResourcePools,
        texture_manager: &mut TextureManager2D,
        mesh_renderer: &MeshRenderer,
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
                    label: data.label.clone(),
                    size: vertex_buffer_combined_size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            );
            let mut staging_buffer = queue.write_buffer_with(
                &pools
                    .buffers
                    .get_resource(&vertex_buffer_combined)
                    .map_err(ResourceManagerError::ResourcePoolError)?
                    .buffer,
                0,
                vertex_buffer_combined_size.try_into().unwrap(),
            );
            staging_buffer[..vertex_buffer_positions_size as usize]
                .copy_from_slice(bytemuck::cast_slice(&data.vertex_positions));
            staging_buffer
                [vertex_buffer_positions_size as usize..vertex_buffer_combined_size as usize]
                .copy_from_slice(bytemuck::cast_slice(&data.vertex_data));
            vertex_buffer_combined
        };

        let index_buffer_size = std::mem::size_of_val(data.indices.as_slice()) as u64;
        let index_buffer = {
            let index_buffer = pools.buffers.alloc(
                device,
                &BufferDesc {
                    label: data.label.clone(),
                    size: index_buffer_size,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                },
            );
            let mut staging_buffer = queue.write_buffer_with(
                &pools
                    .buffers
                    .get_resource(&index_buffer)
                    .map_err(ResourceManagerError::ResourcePoolError)?
                    .buffer,
                0,
                index_buffer_size.try_into().unwrap(),
            );
            staging_buffer[..index_buffer_size as usize]
                .copy_from_slice(bytemuck::cast_slice(&data.indices));
            index_buffer
        };

        let mut materials = SmallVec::with_capacity(data.materials.len());
        for material in &data.materials {
            let texture = texture_manager.get_or_create_gpu_resource(
                pools,
                device,
                queue,
                material.albedo,
            )?;
            let bind_group = pools.bind_groups.alloc(
                device,
                &BindGroupDesc {
                    label: material.label.clone(),
                    entries: smallvec![BindGroupEntry::DefaultTextureView(*texture)],
                    layout: mesh_renderer.bind_group_layout,
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
