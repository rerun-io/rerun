//! Hardware video decoding to `wgpu` textures.
//!
//! ```ignore
//! use re_gpu_video::VideoDeviceSetup;
//!
//! // Before creating the device, probe the adapter.
//! let Some(mut setup) = VideoDeviceSetup::request(&adapter) else {
//!     // No decode support on this adapter, fall back to a software decoder.
//!     return;
//! };
//!
//! // The decoder outputs NV12 textures that get sampled through plane views.
//! descriptor.required_features |= wgpu::Features::TEXTURE_FORMAT_NV12;
//!
//! let (device, queue) = if setup.needs_hal_device_creation() {
//!     // Vulkan needs extra extensions and queues on the device, which only
//!     // wgpu's hal layer can add, through the setup's callback.
//!     unsafe {
//!         let hal_adapter = adapter.as_hal::<wgpu::hal::api::Vulkan>().unwrap();
//!         let open_device = hal_adapter.open_with_callback(
//!             descriptor.required_features,
//!             &descriptor.required_limits,
//!             &descriptor.memory_hints,
//!             Some(setup.create_device_callback()),
//!         )?;
//!         adapter.create_device_from_hal(open_device, &descriptor)?
//!     }
//! } else {
//!     pollster::block_on(adapter.request_device(&descriptor))?
//! };
//!
//! let context = setup.into_context(&device)?;
//!
//! // On a decoder worker thread, never on the render thread:
//! let mut decoder = context.create_h264_decoder()?;
//! for (access_unit, pts) in annex_b_access_units {
//!     for frame in decoder.push_access_unit(access_unit, pts)? {
//!         // `frame.y` and `frame.uv` are the NV12 plane views, ready for sampling.
//!     }
//! }
//! for frame in decoder.flush()? {
//!     // The last frames, once the stream ended.
//! }
//! ```

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
