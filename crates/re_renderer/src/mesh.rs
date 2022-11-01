use std::{num::NonZeroU64, ops::Range};

use smallvec::{smallvec, SmallVec};

use crate::{
    debug_label::DebugLabel,
    resource_pools::{
        buffer_pool::{BufferDesc, BufferHandleStrong},
        WgpuResourcePools,
    },
    RenderContext,
};

/// Mesh vertex as used in gpu residing vertex buffers.
///
/// Needs to be kept in sync with `mesh_vertex.wgsl`.
#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    pub position: glam::Vec3,
    pub normal: glam::Vec3, // TODO(andreas): Compress. Afaik Octahedral Mapping is the best by far, see https://jcgt.org/published/0003/02/01/
    pub texcoord: glam::Vec2,
    // TODO(andreas): More properties? Different kinds of vertices?
}

impl MeshVertex {
    pub const fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshVertex>() as _,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // Normal
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: std::mem::size_of::<f32>() as u64 * 3,
                    shader_location: 1,
                },
                // Texcoord
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: std::mem::size_of::<f32>() as u64 * (3 + 3),
                    shader_location: 2,
                },
            ],
        }
    }
}

pub struct MeshData {
    pub label: DebugLabel,

    // TODO(andreas): Materials
    pub indices: Vec<u32>, // TODO(andreas): different index formats?
    pub vertices: Vec<MeshVertex>,
}

#[derive(Clone)]
pub(crate) struct Mesh {
    // It would be desirable to put both vertex and index buffer into the same buffer, BUT
    // WebGL doesn't allow us to do so! (see https://github.com/gfx-rs/wgpu/pull/3157)
    pub index_buffer: BufferHandleStrong,
    pub vertex_buffer_range: Range<u64>,
    pub vertex_buffer: BufferHandleStrong,
    pub index_buffer_range: Range<u64>,

    /// Every mesh has at least one material.
    pub materials: SmallVec<[Material; 1]>,
}

#[derive(Clone)]
pub(crate) struct Material {
    /// Range of indices in parent mesh that this material covers.
    pub index_range: Range<u32>,
    // todo(andreas): Material properties etc.
    //bind_group: BindGroupHandleStrong,
}

impl Mesh {
    pub fn new(
        pools: &mut WgpuResourcePools,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &MeshData,
    ) -> Self {
        re_log::trace!(
            "uploading new mesh named {:?} with {} vertices and {} triangles",
            data.label.get(),
            data.vertices.len(),
            data.indices.len() / 3
        );

        // TODO(andreas): Have a variant that gets this from a stack allocator.]
        // TODO(andreas): Don't use a queue to upload
        let vertex_buffer_size = (std::mem::size_of::<MeshVertex>() * data.vertices.len()) as u64;
        let index_buffer_size = (std::mem::size_of::<u32>() * data.indices.len()) as u64;

        let vertex_buffer = {
            let vertex_buffer = pools.buffers.alloc(
                device,
                &BufferDesc {
                    label: data.label.clone(),
                    size: vertex_buffer_size,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            );
            let mut staging_buffer = queue.write_buffer_with(
                &pools.buffers.get_resource(&vertex_buffer).unwrap().buffer,
                0,
                NonZeroU64::new(vertex_buffer_size).unwrap(),
            );
            staging_buffer[..vertex_buffer_size as usize]
                .copy_from_slice(bytemuck::cast_slice(&data.vertices));
            vertex_buffer
        };

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
                &pools.buffers.get_resource(&index_buffer).unwrap().buffer,
                0,
                NonZeroU64::new(index_buffer_size).unwrap(),
            );
            staging_buffer[..index_buffer_size as usize]
                .copy_from_slice(bytemuck::cast_slice(&data.indices));
            index_buffer
        };

        Mesh {
            index_buffer,
            vertex_buffer,
            vertex_buffer_range: 0..vertex_buffer_size,
            index_buffer_range: 0..index_buffer_size,

            // TODO(andreas): Actual material support
            materials: smallvec![Material {
                index_range: 0..data.indices.len() as u32,
            }],
        }
    }
}
