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
/// Needs to be kept in sync with `mesh_vertex.wgsl` and `mesh_renderer.rs` pipeline creation.
#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    pub position: glam::Vec3,
    pub normal: glam::Vec3, // TODO(andreas): Compress. Afaik Octahedral Mapping is the best by far, see https://jcgt.org/published/0003/02/01/
    pub texcoord: glam::Vec2,
    // TODO(andreas): More properties? Different kinds of vertices?
}

pub struct MeshData {
    pub label: DebugLabel,

    // TODO(andreas): Materials
    pub indices: Vec<u32>, // TODO(andreas): different index formats?
    pub vertices: Vec<MeshVertex>,
}

#[derive(Clone)]
pub(crate) struct Mesh {
    pub label: DebugLabel,

    /// Combined vertex and index buffer
    /// We *always* have them in the same gpu buffer since the mixed usage generally doesn't seem to be an issue in modern APIs
    pub vertex_and_index_buffer: BufferHandleStrong,
    pub vertex_buffer_range: Range<u64>,
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
        // TODO(andreas): Have a variant that gets this from a stack allocator.]
        let vertex_buffer_size = (std::mem::size_of::<MeshVertex>() * data.vertices.len()) as u64;
        let index_buffer_size = (std::mem::size_of::<u32>() * data.indices.len()) as u64;
        let total_size = vertex_buffer_size + index_buffer_size;

        let vertex_and_index_buffer = pools.buffers.alloc(
            device,
            &BufferDesc {
                label: data.label.clone(),
                size: total_size,
                usage: wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST,
            },
        );

        // TODO(andreas): Don't use a queue to upload
        let mut staging_buffer = queue.write_buffer_with(
            &pools
                .buffers
                .get_resource(&vertex_and_index_buffer)
                .unwrap()
                .buffer,
            0,
            NonZeroU64::new(total_size).unwrap(),
        );
        staging_buffer[..vertex_buffer_size as usize]
            .copy_from_slice(bytemuck::cast_slice(&data.vertices));
        staging_buffer[vertex_buffer_size as usize..total_size as usize]
            .copy_from_slice(bytemuck::cast_slice(&data.indices));

        Mesh {
            label: data.label.clone(),
            vertex_and_index_buffer,
            vertex_buffer_range: 0..vertex_buffer_size,
            index_buffer_range: vertex_buffer_size..total_size,

            // TODO(andreas): Actual material support
            materials: smallvec![Material {
                index_range: 0..data.indices.len() as u32,
            }],
        }
    }
}
