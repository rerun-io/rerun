//! Resource managers are concerned with mapping (typically) higher level user data to
//! their Gpu representation.
//!
//! They facilitate fast & easy gpu upload and resource usage.
//!
//! This is in contrast to the pools in `crate::wgpu_resources` which are exclusively concerned with
//! low level gpu resources and their efficient allocation.

mod mesh_manager;
pub use mesh_manager::{GpuMeshHandle, MeshManager};

mod texture_manager;
pub use texture_manager::{
    GpuTexture2D, Texture2DCreationDesc, TextureManager2D, TextureManager2DError,
};

mod resource_manager;
pub use resource_manager::{ResourceHandle, ResourceLifeTime, ResourceManagerError};
