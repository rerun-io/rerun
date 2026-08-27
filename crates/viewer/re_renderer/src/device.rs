//! Device creation with optional GPU video decode support. Native only.
//!
//! To get video decode support, the Vulkan device has to be created with extra
//! extensions and queues, which only works through wgpu's hal layer.
//! That's why `re_renderer` creates the device itself instead of leaving it to egui-wgpu.

use std::sync::Arc;

pub use re_gpu_video::GpuVideoContext;

use crate::device_caps::DeviceCaps;

/// Creates a device (and queue) for the given adapter the way `re_renderer` wants it,
/// probing for GPU video decode support along the way.
///
/// Video support is best effort and never a reason for device creation to fail:
/// any problem on the video path falls back to plain device creation without it.
pub fn create_device(
    adapter: &wgpu::Adapter,
) -> Result<(wgpu::Device, wgpu::Queue, Option<Arc<GpuVideoContext>>), wgpu::RequestDeviceError> {
    re_tracing::profile_function!();

    let descriptor = DeviceCaps::from_adapter_without_validation(adapter).device_descriptor();

    let Some(mut video_setup) = re_gpu_video::VideoDeviceSetup::request(adapter) else {
        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor))?;
        return Ok((device, queue, None));
    };

    re_log::debug!(
        "GPU video decode support found, H.264: {:?}, H.265: {:?}",
        video_setup.capabilities(re_gpu_video::Codec::H264),
        video_setup.capabilities(re_gpu_video::Codec::H265),
    );

    let device_and_queue = if video_setup.needs_hal_device_creation() {
        create_vulkan_device_with_video(adapter, &descriptor, &mut video_setup)
    } else {
        pollster::block_on(adapter.request_device(&descriptor)).map_err(|err| err.to_string())
    };

    match device_and_queue {
        Ok((device, queue)) => {
            let gpu_video = match video_setup.into_context(&device) {
                Ok(context) => Some(context),
                Err(err) => {
                    re_log::warn!("Failed to set up GPU video decoding: {err}");
                    None
                }
            };
            Ok((device, queue, gpu_video))
        }
        Err(err) => {
            re_log::warn!(
                "Failed to create a device with GPU video decode support, falling back to a device without: {err}"
            );
            let (device, queue) = pollster::block_on(adapter.request_device(&descriptor))?;
            Ok((device, queue, None))
        }
    }
}

/// Creates the device through wgpu's Vulkan hal so the video extensions & queues get enabled.
fn create_vulkan_device_with_video(
    adapter: &wgpu::Adapter,
    descriptor: &wgpu::DeviceDescriptor<'static>,
    video_setup: &mut re_gpu_video::VideoDeviceSetup,
) -> Result<(wgpu::Device, wgpu::Queue), String> {
    let mut descriptor = descriptor.clone();
    // The video decoder outputs NV12 textures which get sampled through plane views.
    descriptor.required_features |= wgpu::Features::TEXTURE_FORMAT_NV12;

    #[expect(unsafe_code)]
    // SAFETY: The hal device is created from this adapter with the features & limits of the
    // descriptor it is then handed to wgpu with, and the setup's callback only adds to the
    // device create info (extensions, queues, feature structs the adapter was probed for).
    unsafe {
        let hal_adapter = adapter
            .as_hal::<wgpu::hal::api::Vulkan>()
            .ok_or_else(|| "adapter is not a Vulkan adapter".to_owned())?;

        let open_device = hal_adapter
            .open_with_callback(
                descriptor.required_features,
                &descriptor.required_limits,
                &descriptor.memory_hints,
                Some(video_setup.create_device_callback()),
            )
            .map_err(|err| format!("hal device creation failed: {err}"))?;

        adapter
            .create_device_from_hal(open_device, &descriptor)
            .map_err(|err| format!("wgpu device creation from hal device failed: {err}"))
    }
}
