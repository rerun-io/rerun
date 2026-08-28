//! Vulkan Video backend.
//!
//! ash/vk types never leave this module tree, see the layering rule in the crate docs.

mod alloc;
mod caps;
mod codec;
mod decoder;
mod device;
mod dpb;
mod output;
mod record;
mod session;
mod sync;

// The safe bitstream parsers, one per codec: pure CPU code producing the plain-data
// `DecodeOp` IR the rest of the backend executes. Fully covered by tests on all platforms.
pub(crate) mod av1;
pub(crate) mod h264;
pub(crate) mod h265;

use std::sync::Arc;

use ash::vk;
use parking_lot::Mutex;

use crate::{Codec, DecodeCapabilities, DecodeError, SetupError};

use caps::{QueuePlan, SupportedCodecs};

pub use decoder::{CpuDecoder, CpuFrame, TextureDecoder};

/// Device extensions needed for video decoding with any codec.
/// Each supported codec adds its own extension on top ([`caps::codec_extension`]).
const BASE_EXTENSIONS: [&std::ffi::CStr; 2] = [
    ash::khr::video_queue::NAME,
    ash::khr::video_decode_queue::NAME,
];

/// Priorities for the queue create infos assembled in the device-creation callback.
///
/// Long enough for the most queues we ever put in one family:
/// wgpu's queue plus the decode and copy queues.
static QUEUE_PRIORITIES: [f32; 3] = [1.0, 0.5, 0.5];

/// Vulkan half of [`crate::VideoDeviceSetup`].
pub struct VulkanSetup {
    queue_plan: QueuePlan,
    codecs: SupportedCodecs,

    /// Chained into the device create info by the device-creation callback.
    /// Owned by the setup so that it outlives the create info's pnext chain.
    ///
    /// Video queue operations require synchronization2, which wgpu itself doesn't enable.
    sync2_features: vk::PhysicalDeviceSynchronization2Features<'static>,
}

impl VulkanSetup {
    /// Probes the adapter for video decode support.
    ///
    /// Returns `None` if anything needed is missing. Never fails device creation:
    /// callers fall back to a plain device without video support.
    pub fn request(adapter: &wgpu::Adapter) -> Option<Self> {
        let probe = caps::probe(adapter)?;

        Some(Self {
            queue_plan: probe.queue_plan,
            codecs: probe.codecs,
            sync2_features: vk::PhysicalDeviceSynchronization2Features::default()
                .synchronization2(true),
        })
    }

    pub fn capabilities(&self, codec: Codec) -> Option<&DecodeCapabilities> {
        self.codecs.get(codec).map(|support| &support.capabilities)
    }

    /// See [`crate::VideoDeviceSetup::create_device_callback`].
    pub fn create_device_callback(&mut self) -> Box<wgpu::hal::vulkan::CreateDeviceCallback<'_>> {
        let queue_plan = self.queue_plan.clone();
        let extensions: Vec<&'static std::ffi::CStr> = BASE_EXTENSIONS
            .into_iter()
            .chain(self.codecs.codecs().map(caps::codec_extension))
            .collect();
        let sync2_features = &mut self.sync2_features;

        Box::new(move |args| {
            for &name in &extensions {
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
                codecs: self.codecs,
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
    pub codecs: SupportedCodecs,
}

/// Vulkan half of [`crate::GpuVideoContext`].
pub struct VulkanContext {
    shared: Arc<Shared>,
}

impl VulkanContext {
    pub fn capabilities(&self, codec: Codec) -> Option<&DecodeCapabilities> {
        self.shared
            .codecs
            .get(codec)
            .map(|support| &support.capabilities)
    }

    /// See [`crate::GpuVideoContext::create_decoder`].
    pub fn create_decoder(&self, codec: Codec) -> Result<TextureDecoder, DecodeError> {
        TextureDecoder::new(self.shared.clone(), codec)
    }

    /// See [`crate::GpuVideoContext::create_cpu_decoder`].
    pub fn create_cpu_decoder(&self, codec: Codec) -> Result<CpuDecoder, DecodeError> {
        CpuDecoder::new(self.shared.clone(), codec)
    }
}
