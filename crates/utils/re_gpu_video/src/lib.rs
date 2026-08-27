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
mod decoder;
mod setup;
mod sorter;
mod vulkan;

pub use context::GpuVideoContext;
pub use decoder::{DecodedFrame, H264Decoder};
pub use setup::VideoDeviceSetup;
pub use vulkan::h264::ParseError;

#[doc(hidden)]
pub use vulkan::{CpuDecoder, CpuFrame};

/// Color interpretation of decoded frames, as declared by the bitstream (SPS VUI).
#[derive(Clone, Copy, Debug, Default)]
pub struct ColorProperties {
    /// Full range (0-255) samples instead of the limited/video range (16-235).
    pub full_range: bool,

    pub matrix_coefficients: MatrixCoefficients,
}

/// YUV→RGB matrix coefficients of [`ColorProperties`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MatrixCoefficients {
    /// The stream doesn't say. Callers pick a default, commonly by resolution.
    #[default]
    Unspecified,

    Bt601,

    Bt709,
}

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

/// Decoding failed. The stream can't be decoded by this backend (or is invalid),
/// or the driver reported an error. Callers fall back to software decoding,
/// silent corruption is never an option.
#[derive(thiserror::Error, Debug)]
pub enum DecodeError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error("Vulkan error: {0}")]
    Vulkan(#[from] ash::vk::Result),

    #[error("Stream exceeds device limits: {0}")]
    ExceedsDeviceLimits(String),

    #[error("The driver reported a decode error (result status {0})")]
    DecodeFailed(i32),
}
