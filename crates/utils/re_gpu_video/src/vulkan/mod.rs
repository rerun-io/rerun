//! Vulkan Video backend.
//!
//! ash/vk types never leave this module tree, see the layering rule in the crate docs.

mod alloc;
mod caps;
mod decoder;
mod device;
mod dpb;
mod output;
mod record;
mod session;
mod sync;

// The safe H.264 bitstream parser: pure CPU code producing the plain-data `DecodeOp` IR
// the rest of the backend executes. Fully covered by tests on all platforms.
pub(crate) mod h264;

use std::sync::Arc;

use ash::vk;
use parking_lot::Mutex;

use crate::{DecodeError, H264DecodeCapabilities, SetupError};

use caps::{QueuePlan, VulkanVideoCaps};

pub use decoder::{CpuDecoder, CpuFrame, TextureDecoder};

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
    pub fn into_context(self, wgpu_device: &wgpu::Device) -> Result<VulkanContext, SetupError> {
        let device = Arc::new(device::Device::from_wgpu(wgpu_device)?);

        let decode_queue = device.get_queue(self.queue_plan.decode);
        let copy_queue = self.queue_plan.copy.map(|copy| device.get_queue(copy));

        Ok(VulkanContext {
            shared: Arc::new(Shared {
                device,
                decode_queue: Mutex::new(decode_queue),
                copy_queue: copy_queue.map(Mutex::new),
                queue_plan: self.queue_plan,
                capabilities: self.capabilities,
                video_caps: self.video_caps,
            }),
        })
    }
}

/// Everything the per-decoder objects share, behind one `Arc`.
pub(crate) struct Shared {
    pub device: Arc<device::Device>,

    pub decode_queue: Mutex<vk::Queue>,

    /// `None` when the copy runs on the decode queue.
    pub copy_queue: Option<Mutex<vk::Queue>>,

    pub queue_plan: QueuePlan,
    pub capabilities: H264DecodeCapabilities,
    pub video_caps: VulkanVideoCaps,
}

/// Vulkan half of [`crate::GpuVideoContext`].
pub struct VulkanContext {
    shared: Arc<Shared>,
}

impl VulkanContext {
    pub fn capabilities(&self) -> &H264DecodeCapabilities {
        &self.shared.capabilities
    }

    /// See [`crate::GpuVideoContext::create_h264_decoder`].
    pub fn create_h264_decoder(&self) -> Result<TextureDecoder, DecodeError> {
        TextureDecoder::new(self.shared.clone())
    }

    /// See [`crate::GpuVideoContext::create_h264_cpu_decoder`].
    pub fn create_h264_cpu_decoder(&self) -> Result<CpuDecoder, DecodeError> {
        CpuDecoder::new(self.shared.clone())
    }
}
