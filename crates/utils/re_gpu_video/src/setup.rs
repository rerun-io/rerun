use std::sync::Arc;

use crate::{GpuVideoContext, H264DecodeCapabilities, SetupError};

/// Everything needed to create a wgpu device with video decode support.
///
/// Obtained from [`Self::request`] before device creation, turned into a [`GpuVideoContext`]
/// with [`Self::into_context`] after the device exists.
pub struct VideoDeviceSetup {
    inner: SetupInner,
}

enum SetupInner {
    Vulkan(crate::vulkan::VulkanSetup),
    // VideoToolbox variant will be added here for the macOS backend.
}

impl VideoDeviceSetup {
    /// Probes the adapter for H.264 decode support, dispatching on its wgpu backend.
    ///
    /// Vulkan: `None` if the video extensions are missing (`MoltenVK`, lavapipe),
    /// there is no H.264-capable decode queue family, or no usable NV12 decode format.
    /// Metal: `None` for now, the `VideoToolbox` backend is not implemented yet.
    /// Other backends: `None`.
    pub fn request(adapter: &wgpu::Adapter) -> Option<Self> {
        re_tracing::profile_function!();

        match adapter.get_info().backend {
            wgpu::Backend::Vulkan => {
                crate::vulkan::VulkanSetup::request(adapter).map(|setup| Self {
                    inner: SetupInner::Vulkan(setup),
                })
            }

            // TODO(isse): VideoToolbox backend on Metal.
            _ => None,
        }
    }

    pub fn capabilities(&self) -> &H264DecodeCapabilities {
        match &self.inner {
            SetupInner::Vulkan(setup) => setup.capabilities(),
        }
    }

    /// True only for the Vulkan backend: device creation must go through
    /// [`wgpu::hal::vulkan::Adapter::open_with_callback`] with the callback from
    /// [`Self::create_device_callback`].
    ///
    /// Metal works against a plainly created device.
    pub fn needs_hal_device_creation(&self) -> bool {
        match &self.inner {
            SetupInner::Vulkan(_) => true,
        }
    }

    /// Vulkan only, see [`Self::needs_hal_device_creation`].
    ///
    /// The returned callback adds the video extensions, the decode and copy queues,
    /// and the required feature structs to the device create info.
    ///
    /// # Panics
    ///
    /// Panics for backends where [`Self::needs_hal_device_creation`] is false.
    pub fn create_device_callback(&mut self) -> Box<wgpu::hal::vulkan::CreateDeviceCallback<'_>> {
        match &mut self.inner {
            SetupInner::Vulkan(setup) => setup.create_device_callback(),
        }
    }

    /// Builds the [`GpuVideoContext`] against the created device.
    ///
    /// Vulkan: the device must have been created through the callback from
    /// [`Self::create_device_callback`], on the same adapter that was probed.
    pub fn into_context(self, device: &wgpu::Device) -> Result<Arc<GpuVideoContext>, SetupError> {
        match self.inner {
            SetupInner::Vulkan(setup) => Ok(Arc::new(GpuVideoContext::new_vulkan(
                setup.into_context(device)?,
            ))),
        }
    }
}
