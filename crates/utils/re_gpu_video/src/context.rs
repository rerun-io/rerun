use crate::{Codec, DecodeCapabilities, DecodeError};

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

    /// The device's decode capabilities for a codec, `None` when it can't decode it.
    pub fn capabilities(&self, codec: Codec) -> Option<&DecodeCapabilities> {
        match &self.inner {
            ContextInner::Vulkan(context) => context.capabilities(codec),
        }
    }

    /// Short name of the backend in use, for logging.
    pub fn backend_name(&self) -> &'static str {
        match &self.inner {
            ContextInner::Vulkan(_) => "Vulkan Video",
        }
    }

    /// Creates a decoder producing frames as GPU textures.
    pub fn create_decoder(&self, codec: Codec) -> Result<crate::Decoder, DecodeError> {
        match &self.inner {
            ContextInner::Vulkan(context) => {
                Ok(crate::Decoder::new_vulkan(context.create_decoder(codec)?))
            }
        }
    }

    /// Creates a decoder reading the decoded frames back into CPU pixel buffers.
    ///
    /// The permanent debugging path (see `examples/decode_to_yuv.rs`): the texture
    /// decoder is what real callers use. Vulkan backend only.
    #[doc(hidden)]
    pub fn create_cpu_decoder(&self, codec: Codec) -> Result<crate::CpuDecoder, DecodeError> {
        match &self.inner {
            ContextInner::Vulkan(context) => context.create_cpu_decoder(codec),
        }
    }
}
