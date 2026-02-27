//! Resource managers are concerned with mapping (typically) higher level user data to
//! their Gpu representation.
//!
//! They facilitate fast & easy gpu upload and resource usage.
//!
//! This is in contrast to the pools in `crate::wgpu_resources` which are exclusively concerned with
//! low level gpu resources and their efficient allocation.

mod image_data_to_texture;
mod texture_manager;
mod texture_manager_3d;
mod yuv_converter;

pub use image_data_to_texture::{
    ImageDataDesc, ImageDataToTextureError, SourceImageDataFormat, transfer_image_data_to_texture,
};
pub use texture_manager::{
    AlphaChannelUsage, GpuTexture2D, TextureManager2D, TextureManager2DError,
};
pub use texture_manager_3d::{GpuTexture3D, TextureManager3D, VolumeDataDesc};
pub use yuv_converter::{YuvMatrixCoefficients, YuvPixelLayout, YuvRange};
