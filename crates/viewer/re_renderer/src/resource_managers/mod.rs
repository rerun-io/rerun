//! Resource managers are concerned with mapping (typically) higher level user data to
//! their Gpu representation.
//!
//! They facilitate fast & easy gpu upload and resource usage.
//!
//! This is in contrast to the pools in `crate::wgpu_resources` which are exclusively concerned with
//! low level gpu resources and their efficient allocation.

mod texture_manager;
pub use texture_manager::{
    GpuTexture2D, Texture2DCreationDesc, TextureCreationError, TextureManager2D,
    TextureManager2DError,
};
