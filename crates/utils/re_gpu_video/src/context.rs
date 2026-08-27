use crate::{DecodeError, H264DecodeCapabilities};

/// Video decode support living alongside a wgpu device.
///
/// Created via [`crate::VideoDeviceSetup::into_context`].
/// Holds the decode queues and backend function tables, and is the factory for decoders.
pub struct GpuVideoContext {
    inner: ContextInner,
}

enum ContextInner {
    Vulkan(crate::vulkan::VulkanContext),
    // VideoToolbox variant will be added here for the macOS backend.
}

impl GpuVideoContext {
    pub(crate) fn new_vulkan(context: crate::vulkan::VulkanContext) -> Self {
        Self {
            inner: ContextInner::Vulkan(context),
        }
    }

    pub fn h264_capabilities(&self) -> &H264DecodeCapabilities {
        match &self.inner {
            ContextInner::Vulkan(context) => context.capabilities(),
        }
    }

    /// Short name of the backend in use, for logging.
    pub fn backend_name(&self) -> &'static str {
        match &self.inner {
            ContextInner::Vulkan(_) => "Vulkan Video",
        }
    }

    /// Creates a decoder producing frames as GPU textures.
    pub fn create_h264_decoder(&self) -> Result<crate::H264Decoder, DecodeError> {
        match &self.inner {
            ContextInner::Vulkan(context) => Ok(crate::H264Decoder::new_vulkan(
                context.create_h264_decoder()?,
            )),
        }
    }

    /// Creates a decoder reading the decoded frames back into CPU pixel buffers.
    ///
    /// The permanent debugging path (see `examples/decode_to_yuv.rs`): the texture
    /// decoder of later milestones is what real callers use. Vulkan backend only.
    #[doc(hidden)]
    pub fn create_h264_cpu_decoder(&self) -> Result<crate::CpuDecoder, DecodeError> {
        match &self.inner {
            ContextInner::Vulkan(context) => context.create_h264_cpu_decoder(),
        }
    }
}
