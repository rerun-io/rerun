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
pub use decoder::{DecodedFrame, Decoder};
pub use setup::VideoDeviceSetup;

/// A video codec decodable by this crate, hardware support permitting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    H264,
    H265,
    AV1,
}

impl std::fmt::Display for Codec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::H264 => "H.264",
            Self::H265 => "H.265",
            Self::AV1 => "AV1",
        })
    }
}

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

/// Decode capabilities of a device for one [`Codec`], as reported by the backend.
#[derive(Clone, Debug)]
pub struct DecodeCapabilities {
    /// Smallest supported coded width & height.
    pub min_coded_extent: [u32; 2],

    /// Largest supported coded width & height.
    pub max_coded_extent: [u32; 2],

    /// Maximum number of decoded-picture-buffer slots.
    pub max_dpb_slots: u32,

    /// Maximum number of active reference pictures per decode operation.
    pub max_active_references: u32,

    /// Maximum supported level, in the codec's own `level_idc` numbering
    /// (e.g. 51 for H.264 level 5.1).
    pub max_level_idc: u32,
}

/// The pushed data can't be decoded. The decoder gives up and falls back to
/// software decoding, silent corruption is never an option.
#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("Data is not an annex-b NAL stream")]
    NotAnnexB,

    // `h264-reader` errors implement neither `Display` nor `Error`,
    // `cros-codecs` reports plain strings.
    #[error("Failed to parse {what}: {details}")]
    Nal { what: &'static str, details: String },

    #[error("Unsupported bitstream feature: {0}")]
    Unsupported(&'static str),

    #[error("Invalid bitstream: {0}")]
    Invalid(&'static str),

    #[error("Slice header picture order count syntax doesn't match the SPS")]
    PocSyntaxMismatch,

    #[error("The stream needs {needed} DPB slots, the device supports {available}")]
    TooManyRefFrames { needed: u32, available: u8 },

    #[error(
        "Gap in frame_num: got {got}, expected {expected} — reference frames are missing \
         (gaps_in_frame_num_value_allowed_flag: {gaps_allowed})"
    )]
    FrameNumGap {
        got: u16,
        expected: u16,
        gaps_allowed: bool,
    },

    #[error("A P or B frame arrived while no reference frames are available")]
    NoReferencesAvailable,

    #[error("Missing {what}")]
    MissingReference { what: &'static str },

    #[error("More reference frames than the stream declared in its SPS")]
    DpbOverflow,

    #[error("The access unit starts in the middle of a frame")]
    IncompletePicture,

    #[error("Slices within one frame disagree on their shared header fields")]
    InconsistentSlices,

    #[error("Expected a random access point (start of stream, after a seek, or after an error)")]
    ExpectedRandomAccessPoint,

    #[error("The stream applies film grain, which this device's AV1 decoder does not support")]
    FilmGrainUnsupported,
}

impl ParseError {
    pub(crate) fn nal(what: &'static str, err: impl std::fmt::Debug) -> Self {
        Self::Nal {
            what,
            details: format!("{err:?}"),
        }
    }
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

    #[error("The device does not support GPU decoding of {0}")]
    UnsupportedCodec(Codec),
}
