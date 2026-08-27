//! Hardware video decoding straight to `wgpu` textures, without the decoded frames ever leaving the GPU.
//!
//! Two backends, chosen at runtime based on the wgpu backend of the adapter
//! (see [`VideoDeviceSetup::request`]):
//! * Vulkan Video: any Vulkan driver exposing the video decode extensions.
//!   Software rasterizers and `MoltenVK` don't, the probe reports no support there.
//! * `VideoToolbox` (macOS, not yet implemented)
//!
//! Integration happens in two steps:
//! * At device-creation time, [`VideoDeviceSetup::request`] probes the adapter for decode support.
//!   On Vulkan, the wgpu device must then be created through
//!   [`wgpu::hal::vulkan::Adapter::open_with_callback`] with the callback from
//!   [`VideoDeviceSetup::create_device_callback`], so that the video extensions and queues
//!   get enabled on it.
//! * [`VideoDeviceSetup::into_context`] then turns the setup into a [`GpuVideoContext`],
//!   from which decoders can be created.
//!
//! This crate is native only, web builds must not include it.

mod context;
mod setup;
mod vulkan;

pub use context::GpuVideoContext;
pub use setup::VideoDeviceSetup;

/// H.264 decode capabilities of a device, as reported by the backend.
#[derive(Clone, Debug)]
pub struct H264DecodeCapabilities {
    /// Smallest supported coded width & height.
    pub min_coded_extent: [u32; 2],

    /// Largest supported coded width & height.
    pub max_coded_extent: [u32; 2],

    /// Maximum number of decoded-picture-buffer slots.
    pub max_dpb_slots: u32,

    /// Maximum number of active reference pictures per decode operation.
    pub max_active_references: u32,

    /// Maximum supported H.264 level (`level_idc` value, e.g. 51 for level 5.1).
    pub max_level_idc: u32,
}

/// Failed to turn a [`VideoDeviceSetup`] into a [`GpuVideoContext`].
#[derive(thiserror::Error, Debug)]
pub enum SetupError {
    #[error("The wgpu device does not belong to the expected backend")]
    UnexpectedWgpuBackend,

    #[error("Vulkan error: {0}")]
    Vulkan(#[from] ash::vk::Result),
}
