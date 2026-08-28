use std::sync::Arc;

use crate::{Codec, DecodeCapabilities, GpuVideoContext, SetupError};

/// Everything needed to create a wgpu device with video decode support.
///
/// Obtained from [`Self::request`] before device creation, turned into a [`GpuVideoContext`]
/// with [`Self::into_context`] after the device exists.
pub struct VideoDeviceSetup {
    inner: SetupInner,
}

enum SetupInner {
    Vulkan(Box<crate::vulkan::VulkanSetup>),

    #[cfg(target_os = "macos")]
    VideoToolbox(crate::videotoolbox::VideoToolboxSetup),
}

impl VideoDeviceSetup {
    /// Probes the adapter for video decode support, dispatching on its wgpu backend.
    ///
    /// Vulkan: `None` if the video extensions are missing (`MoltenVK`, lavapipe),
    /// there is no capable decode queue family, or no usable NV12 decode format.
    /// Metal: always `Some`, `VideoToolbox` decodes H.264 and H.265 on every Mac.
    /// Other backends: `None`.
    pub fn request(adapter: &wgpu::Adapter) -> Option<Self> {
        re_tracing::profile_function!();

        match adapter.get_info().backend {
            wgpu::Backend::Vulkan => {
                crate::vulkan::VulkanSetup::request(adapter).map(|setup| Self {
                    inner: SetupInner::Vulkan(Box::new(setup)),
                })
            }

            #[cfg(target_os = "macos")]
            wgpu::Backend::Metal => {
                crate::videotoolbox::VideoToolboxSetup::request(adapter).map(|setup| Self {
                    inner: SetupInner::VideoToolbox(setup),
                })
            }

            _ => None,
        }
    }

    /// The device's decode capabilities for a codec, `None` when it can't decode it.
    pub fn capabilities(&self, codec: Codec) -> Option<&DecodeCapabilities> {
        match &self.inner {
            SetupInner::Vulkan(setup) => setup.capabilities(codec),

            #[cfg(target_os = "macos")]
            SetupInner::VideoToolbox(setup) => setup.capabilities(codec),
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

            #[cfg(target_os = "macos")]
            SetupInner::VideoToolbox(_) => false,
        }
    }

    /// Vulkan only, see [`Self::needs_hal_device_creation`].
    ///
    /// The returned callback pushes the video extensions, queue create infos for the decode
    /// and copy queues, and required feature structs onto the device create info.
    ///
    /// Borrows `self` mutably so that structs owned by the setup outlive the create info's
    /// pnext chain.
    ///
    /// # Panics
    ///
    /// Panics for backends where [`Self::needs_hal_device_creation`] is false.
    pub fn create_device_callback(&mut self) -> Box<wgpu::hal::vulkan::CreateDeviceCallback<'_>> {
        match &mut self.inner {
            SetupInner::Vulkan(setup) => setup.create_device_callback(),

            #[cfg(target_os = "macos")]
            SetupInner::VideoToolbox(_) => {
                panic!("the VideoToolbox backend works against a plainly created device")
            }
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

            #[cfg(target_os = "macos")]
            SetupInner::VideoToolbox(setup) => Ok(Arc::new(GpuVideoContext::new_video_toolbox(
                setup.into_context(device)?,
            ))),
        }
    }
}
