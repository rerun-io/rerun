use slotmap::{DefaultKey, Key, SlotMap};

use crate::{
    mesh::{Mesh, MeshData},
    RenderContext,
};

/// Handle to a mesh that is stored in the [`MeshManager`]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MeshHandle {
    /// Mesh handle that is valid until user explicitly removes the mesh from [`MeshManager`].
    LongLived(DefaultKey),

    /// Mesh handle that is valid for a single frame
    Frame {
        key: DefaultKey,
        /// This handle is only valid for this frame.
        /// Querying it during any other frame will fail.
        valid_frame_index: u64,
    },
}

#[derive(Default)]
pub struct MeshManager {
    long_lived_meshes: SlotMap<DefaultKey, Mesh>,
    frame_meshes: SlotMap<DefaultKey, Mesh>,
    frame_index: u64,
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum MeshManagerError {
    #[error("The requested mesh is no longer valid. It was valid for the frame index {current_frame_index}, but the current frame index is {valid_frame_index}")]
    ExpiredMesh {
        current_frame_index: u64,
        valid_frame_index: u64,
    },

    #[error("The requested mesh isn't available because the handle is no longer valid")]
    MeshNotAvailable,

    #[error("The passed resource handle was null")]
    NullHandle,
}

impl MeshManager {
    /// Creates a new, long lived mesh.
    ///
    /// Memory will be reclaimed once all (strong) handles are dropped
    /// For short lived meshes use [`new_frame_mesh`] as it has more efficient resource usage for this scenario.
    pub fn new_long_lived_mesh(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &MeshData,
    ) -> MeshHandle {
        let key = ctx.meshes.long_lived_meshes.insert(Mesh::new(
            &mut ctx.resource_pools,
            device,
            queue,
            data,
        ));
        MeshHandle::LongLived(key)
    }

    /// Creates a mesh that lives for the duration of the frame
    ///
    /// Using the handle in the following frame will cause an error.
    pub fn new_frame_mesh(
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &MeshData,
    ) -> MeshHandle {
        let key =
            ctx.meshes
                .frame_meshes
                .insert(Mesh::new(&mut ctx.resource_pools, device, queue, data));
        MeshHandle::Frame {
            key,
            valid_frame_index: ctx.meshes.frame_index,
        }
    }

    /// Retrieve a mesh.
    pub(crate) fn get_mesh(&self, handle: MeshHandle) -> Result<&Mesh, MeshManagerError> {
        let (slotmap, key) = match handle {
            MeshHandle::LongLived(key) => (&self.long_lived_meshes, key),
            MeshHandle::Frame {
                key,
                valid_frame_index,
            } => {
                if valid_frame_index != self.frame_index {
                    return Err(MeshManagerError::ExpiredMesh {
                        current_frame_index: self.frame_index,
                        valid_frame_index,
                    });
                }
                (&self.frame_meshes, key)
            }
        };

        slotmap.get(key).ok_or_else(|| {
            if key.is_null() {
                MeshManagerError::NullHandle
            } else {
                MeshManagerError::MeshNotAvailable
            }
        })
    }

    pub(crate) fn frame_maintenance(&mut self, frame_index: u64) {
        self.frame_meshes.clear();
        self.frame_index = frame_index;
    }
}
