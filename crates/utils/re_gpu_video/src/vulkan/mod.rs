//! Vulkan Video backend.
//!
//! ash/vk types never leave this module tree, see the layering rule in the crate docs.

mod caps;

use ash::vk;
use parking_lot::Mutex;

use crate::{H264DecodeCapabilities, SetupError};

use caps::{QueuePlan, VulkanVideoCaps};

/// Device extensions needed for H.264 decoding.
const REQUIRED_EXTENSIONS: [&std::ffi::CStr; 3] = [
    ash::khr::video_queue::NAME,
    ash::khr::video_decode_queue::NAME,
    ash::khr::video_decode_h264::NAME,
];

/// Priorities for the queue create infos assembled in the device-creation callback.
///
/// Long enough for the most queues we ever put in one family:
/// wgpu's queue plus the decode and copy queues.
static QUEUE_PRIORITIES: [f32; 3] = [1.0, 0.5, 0.5];

/// Vulkan half of [`crate::VideoDeviceSetup`].
pub struct VulkanSetup {
    queue_plan: QueuePlan,
    capabilities: H264DecodeCapabilities,
    video_caps: VulkanVideoCaps,

    /// Chained into the device create info by the device-creation callback.
    /// Owned by the setup so that it outlives the create info's pnext chain.
    ///
    /// Video queue operations require synchronization2, which wgpu itself doesn't enable.
    sync2_features: vk::PhysicalDeviceSynchronization2Features<'static>,
}

impl VulkanSetup {
    /// Probes the adapter for H.264 decode support.
    ///
    /// Returns `None` if anything needed is missing. Never fails device creation:
    /// callers fall back to a plain device without video support.
    pub fn request(adapter: &wgpu::Adapter) -> Option<Self> {
        let probe = caps::probe(adapter)?;

        Some(Self {
            queue_plan: probe.queue_plan,
            capabilities: probe.capabilities,
            video_caps: probe.video_caps,
            sync2_features: vk::PhysicalDeviceSynchronization2Features::default()
                .synchronization2(true),
        })
    }

    pub fn capabilities(&self) -> &H264DecodeCapabilities {
        &self.capabilities
    }

    /// See [`crate::VideoDeviceSetup::create_device_callback`].
    pub fn create_device_callback(&mut self) -> Box<wgpu::hal::vulkan::CreateDeviceCallback<'_>> {
        let queue_plan = self.queue_plan.clone();
        let sync2_features = &mut self.sync2_features;

        Box::new(move |args| {
            for name in REQUIRED_EXTENSIONS {
                if !args.extensions.contains(&name) {
                    args.extensions.push(name);
                }
            }

            for &(family_index, queue_count) in &queue_plan.queue_counts {
                let priorities = &QUEUE_PRIORITIES[..queue_count as usize];
                if let Some(create_info) = args
                    .queue_create_infos
                    .iter_mut()
                    .find(|info| info.queue_family_index == family_index)
                {
                    // wgpu already requests one queue in this family (its own),
                    // bump the count. `queue_counts` includes wgpu's queue.
                    *create_info = create_info.queue_priorities(priorities);
                } else {
                    args.queue_create_infos.push(
                        vk::DeviceQueueCreateInfo::default()
                            .queue_family_index(family_index)
                            .queue_priorities(priorities),
                    );
                }
            }

            *args.create_info = args.create_info.push_next(sync2_features);
        })
    }

    /// See [`crate::VideoDeviceSetup::into_context`].
    ///
    /// Fetches the decode & copy queues from the raw device and builds the
    /// video extension function tables.
    pub fn into_context(self, device: &wgpu::Device) -> Result<VulkanContext, SetupError> {
        #[expect(unsafe_code)] // Safety: the device is kept alive by the returned context.
        let (raw_device, video_queue_fns, video_decode_fns, decode_queue, copy_queue) = unsafe {
            let hal_device = device
                .as_hal::<wgpu::hal::api::Vulkan>()
                .ok_or(SetupError::UnexpectedWgpuBackend)?;

            let raw_device = hal_device.raw_device().clone();
            let raw_instance = hal_device.shared_instance().raw_instance();

            let video_queue_fns = ash::khr::video_queue::Device::new(raw_instance, &raw_device);
            let video_decode_fns =
                ash::khr::video_decode_queue::Device::new(raw_instance, &raw_device);

            let decode = self.queue_plan.decode;
            let decode_queue = raw_device.get_device_queue(decode.family_index, decode.queue_index);
            let copy_queue = self
                .queue_plan
                .copy
                .map(|copy| raw_device.get_device_queue(copy.family_index, copy.queue_index));

            (
                raw_device,
                video_queue_fns,
                video_decode_fns,
                decode_queue,
                copy_queue,
            )
        };

        Ok(VulkanContext {
            device: device.clone(),
            raw_device,
            video_queue_fns,
            video_decode_fns,
            decode_queue: Mutex::new(decode_queue),
            copy_queue: copy_queue.map(Mutex::new),
            capabilities: self.capabilities,
            video_caps: self.video_caps,
        })
    }
}

/// Vulkan half of [`crate::GpuVideoContext`].
pub struct VulkanContext {
    /// Keeps the wgpu device (and with it the raw Vulkan device) alive as long as this context.
    #[expect(dead_code)] // Used from the decoder milestones on.
    device: wgpu::Device,

    #[expect(dead_code)] // Used from the decoder milestones on.
    raw_device: ash::Device,

    #[expect(dead_code)] // Used from the decoder milestones on.
    video_queue_fns: ash::khr::video_queue::Device,

    #[expect(dead_code)] // Used from the decoder milestones on.
    video_decode_fns: ash::khr::video_decode_queue::Device,

    #[expect(dead_code)] // Used from the decoder milestones on.
    decode_queue: Mutex<vk::Queue>,

    /// `None` when the copy runs on the decode queue.
    #[expect(dead_code)] // Used from the decoder milestones on.
    copy_queue: Option<Mutex<vk::Queue>>,

    capabilities: H264DecodeCapabilities,

    #[expect(dead_code)] // Used from the decoder milestones on.
    video_caps: VulkanVideoCaps,
}

impl VulkanContext {
    pub fn capabilities(&self) -> &H264DecodeCapabilities {
        &self.capabilities
    }
}
