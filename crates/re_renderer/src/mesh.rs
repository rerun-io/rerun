use std::{num::NonZeroU64, ops::Range};

use smallvec::{smallvec, SmallVec};

use crate::{
    debug_label::DebugLabel,
    resource_pools::buffer_pool::{BufferDesc, BufferHandleStrong},
    RenderContext,
};

/// Mesh vertex as used in gpu residing vertex buffers.
#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    position: glam::Vec3,
    texcoord: glam::Vec2,
    // TODO(andreas): More properties? Different kinds of vertices?
}

pub struct MeshData {
    label: DebugLabel,

    // TODO(andreas): Materials
    indices: Vec<u32>, // TODO(andreas): different index formats?
    vertices: Vec<MeshVertex>,
}

pub(crate) struct Mesh {
    label: DebugLabel,

    /// Combined vertex and index buffer
    /// We *always* have them in the same gpu buffer since the mixed usage generally doesn't seem to be an issue in modern APIs
    vertex_and_index_buffer: BufferHandleStrong,
    vertex_buffer_range: Range<u64>,
    index_buffer_range: Range<u64>,

    /// Every mesh has at least one material.
    materials: SmallVec<[Material; 1]>,
}

pub(crate) struct Material {
    label: DebugLabel,

    /// Range of indices in parent mesh that this material covers.
    index_range: Range<u32>,
    // todo(andreas): Material properties etc.
    //bind_group: BindGroupHandleStrong,
}

impl Mesh {
    pub fn new(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &MeshData,
    ) -> Self {
        // TODO(andreas): Have a variant that gets this from a stack allocator.]
        let vertex_buffer_size = (std::mem::size_of::<MeshVertex>() * data.vertices.len()) as u64;
        let index_buffer_size = (std::mem::size_of::<u32>() * data.indices.len()) as u64;
        let total_size = vertex_buffer_size + index_buffer_size;

        let vertex_and_index_buffer = ctx.resource_pools.buffers.alloc(
            device,
            &BufferDesc {
                label: data.label.clone(),
                size: total_size,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::VERTEX,
            },
        );

        // TODO(andreas): Don't use a queue to upload
        let mut staging_buffer = queue.write_buffer_with(
            &ctx.resource_pools
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
                label: data.label.clone(),
                index_range: 0..data.indices.len() as u32,
            }],
        }
    }
}
