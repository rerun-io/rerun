//! `VideoToolbox` backend, used whenever wgpu runs on Metal.
//!
//! objc2 and `CoreMedia` types never leave this module tree, see the layering rule in
//! the crate docs. There is no device surgery here: `VideoToolbox` decodes into
//! `IOSurface`-backed pixel buffers, and Metal wraps those as textures against the
//! device wgpu created on its own.

mod decoder;
mod nalu;
mod output;

#[cfg(test)]
mod tests;

use objc2_core_foundation::CFRetained;
use objc2_core_media::{kCMVideoCodecType_H264, kCMVideoCodecType_HEVC};
use objc2_core_video::CVPixelBuffer;
use objc2_video_toolbox::VTIsHardwareDecodeSupported;

use crate::{Codec, DecodeCapabilities, DecodeError, SetupError};

pub use decoder::VideoToolboxDecoder;

/// The codecs the backend decodes, in the order capabilities are reported for them.
///
/// AV1 is missing on purpose: its format description needs the `av1C` configuration
/// record, which the annex-b-shaped decoder API never sees.
const CODECS: [Codec; 2] = [Codec::H264, Codec::H265];

/// A decoded frame's pixel buffer, moved between `VideoToolbox`'s decode threads,
/// the decoder and the textures wrapping it.
#[derive(Clone)]
pub(crate) struct PixelBuffer(CFRetained<CVPixelBuffer>);

// SAFETY: `CVPixelBuffer` is a reference counted CoreVideo type with thread safe
// retain/release, and nothing here mutates the buffer.
#[expect(unsafe_code)]
unsafe impl Send for PixelBuffer {}
// SAFETY: See above.
#[expect(unsafe_code)]
unsafe impl Sync for PixelBuffer {}

impl PixelBuffer {
    fn new(buffer: CFRetained<CVPixelBuffer>) -> Self {
        Self(buffer)
    }

    fn get(&self) -> &CVPixelBuffer {
        &self.0
    }
}

/// `VideoToolbox` half of [`crate::VideoDeviceSetup`].
///
/// There is nothing to prepare for device creation, only the capability snapshot:
/// the backend works against a plainly created wgpu device.
pub struct VideoToolboxSetup {
    codecs: SupportedCodecs,
}

impl VideoToolboxSetup {
    /// Probes the system for decode support.
    ///
    /// `VideoToolbox` decodes H.264 and H.265 on every Mac, in software where the
    /// hardware can't, which still beats the round trip through the `FFmpeg` CLI.
    #[expect(clippy::unnecessary_wraps, reason = "mirrors the Vulkan probe")]
    pub fn request(_adapter: &wgpu::Adapter) -> Option<Self> {
        Some(Self {
            codecs: SupportedCodecs::probe(),
        })
    }

    pub fn capabilities(&self, codec: Codec) -> Option<&DecodeCapabilities> {
        self.codecs.get(codec)
    }

    /// See [`crate::VideoDeviceSetup::into_context`].
    pub fn into_context(
        self,
        wgpu_device: &wgpu::Device,
    ) -> Result<VideoToolboxContext, SetupError> {
        #[expect(unsafe_code)]
        // SAFETY: The hal device is only looked at, never used past this block.
        let is_metal = unsafe { wgpu_device.as_hal::<wgpu::hal::api::Metal>().is_some() };
        if !is_metal {
            return Err(SetupError::UnexpectedWgpuBackend);
        }

        Ok(VideoToolboxContext {
            wgpu_device: wgpu_device.clone(),
            codecs: self.codecs,
        })
    }
}

/// `VideoToolbox` half of [`crate::GpuVideoContext`].
pub struct VideoToolboxContext {
    wgpu_device: wgpu::Device,
    codecs: SupportedCodecs,
}

impl VideoToolboxContext {
    pub fn capabilities(&self, codec: Codec) -> Option<&DecodeCapabilities> {
        self.codecs.get(codec)
    }

    /// See [`crate::GpuVideoContext::create_decoder`].
    pub fn create_decoder(&self, codec: Codec) -> Result<VideoToolboxDecoder, DecodeError> {
        if self.codecs.get(codec).is_none() {
            return Err(DecodeError::UnsupportedCodec(codec));
        }
        VideoToolboxDecoder::new(self.wgpu_device.clone(), codec)
    }
}

/// The probed decode support per codec.
struct SupportedCodecs {
    entries: Vec<(Codec, DecodeCapabilities)>,
}

impl SupportedCodecs {
    fn probe() -> Self {
        let entries = CODECS
            .into_iter()
            .map(|codec| {
                let codec_type = match codec {
                    Codec::H264 => kCMVideoCodecType_H264,
                    Codec::H265 => kCMVideoCodecType_HEVC,
                    Codec::AV1 => unreachable!("not in CODECS"),
                };
                #[expect(unsafe_code)]
                // SAFETY: The call takes a plain codec type and touches nothing else.
                let hardware_accelerated = unsafe { VTIsHardwareDecodeSupported(codec_type) };

                (
                    codec,
                    DecodeCapabilities {
                        // VideoToolbox has no capability query beyond this one.
                        min_coded_extent: None,
                        max_coded_extent: None,
                        max_level_idc: None,
                        hardware_accelerated,
                    },
                )
            })
            .collect();

        Self { entries }
    }

    fn get(&self, codec: Codec) -> Option<&DecodeCapabilities> {
        self.entries
            .iter()
            .find(|(supported, _)| *supported == codec)
            .map(|(_, capabilities)| capabilities)
    }
}
