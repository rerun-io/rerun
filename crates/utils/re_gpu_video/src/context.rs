use crate::H264DecodeCapabilities;

/// Video decode support living alongside a wgpu device.
///
/// Created via [`crate::VideoDeviceSetup::into_context`].
/// Holds the decode queues and backend function tables, and is the factory for decoders.
pub struct GpuVideoContext {
    inner: ContextInner,
}

enum ContextInner {
    #[cfg(vulkan_video)]
    Vulkan(crate::vulkan::VulkanContext),
    // VideoToolbox variant will be added here for the macOS backend.
}

impl GpuVideoContext {
    #[cfg(vulkan_video)]
    pub(crate) fn new_vulkan(context: crate::vulkan::VulkanContext) -> Self {
        Self {
            inner: ContextInner::Vulkan(context),
        }
    }

    pub fn h264_capabilities(&self) -> &H264DecodeCapabilities {
        match &self.inner {
            #[cfg(vulkan_video)]
            ContextInner::Vulkan(context) => context.capabilities(),

            #[cfg(not(vulkan_video))]
            _ => unreachable!("`GpuVideoContext` cannot be constructed without a backend"),
        }
    }

    /// Short name of the backend in use, for logging.
    pub fn backend_name(&self) -> &'static str {
        match &self.inner {
            #[cfg(vulkan_video)]
            ContextInner::Vulkan(_) => "Vulkan Video",

            #[cfg(not(vulkan_video))]
            _ => unreachable!("`GpuVideoContext` cannot be constructed without a backend"),
        }
    }
}
