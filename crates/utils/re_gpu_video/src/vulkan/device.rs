//! The shared raw-device wrapper everything in the backend allocates and records through.

use std::sync::Arc;

use ash::vk;

use crate::SetupError;

/// Owned Vulkan handles and function tables shared by the whole backend.
///
/// Never destroys the raw device: wgpu owns it, and the held [`wgpu::Device`] clone
/// keeps it alive for as long as anything references this wrapper.
pub struct Device {
    /// Keeps the wgpu device (and with it the raw Vulkan device) alive.
    _wgpu_device: wgpu::Device,

    pub raw: ash::Device,
    pub video_queue_fns: ash::khr::video_queue::Device,
    pub video_decode_fns: ash::khr::video_decode_queue::Device,
    pub memory_properties: vk::PhysicalDeviceMemoryProperties,
}

impl Device {
    /// Pulls the raw handles out of a wgpu device created on the Vulkan backend.
    #[expect(unsafe_code)]
    pub fn from_wgpu(device: &wgpu::Device) -> Result<Self, SetupError> {
        // SAFETY: Nothing here outlives the guard except the cloned handles,
        // which the held wgpu device keeps alive.
        unsafe {
            let hal_device = device
                .as_hal::<wgpu::hal::api::Vulkan>()
                .ok_or(SetupError::UnexpectedWgpuBackend)?;

            let raw = hal_device.raw_device().clone();
            let raw_instance = hal_device.shared_instance().raw_instance();
            let memory_properties = raw_instance
                .get_physical_device_memory_properties(hal_device.raw_physical_device());

            Ok(Self {
                _wgpu_device: device.clone(),
                video_queue_fns: ash::khr::video_queue::Device::new(raw_instance, &raw),
                video_decode_fns: ash::khr::video_decode_queue::Device::new(raw_instance, &raw),
                raw,
                memory_properties,
            })
        }
    }

    /// Fetches a queue created through the device-creation callback.
    #[expect(unsafe_code)]
    pub fn get_queue(&self, slot: super::caps::QueueSlot) -> vk::Queue {
        // SAFETY: The queue was requested at device creation per the queue plan.
        unsafe {
            self.raw
                .get_device_queue(slot.family_index, slot.queue_index)
        }
    }
}

/// A command pool with a single primary command buffer, re-recorded every use.
pub struct CommandPool {
    device: Arc<Device>,
    pool: vk::CommandPool,
    pub buffer: vk::CommandBuffer,
}

impl CommandPool {
    #[expect(unsafe_code)]
    pub fn new(device: Arc<Device>, queue_family_index: u32) -> Result<Self, vk::Result> {
        let pool_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::TRANSIENT)
            .queue_family_index(queue_family_index);

        // SAFETY: The buffer is allocated from the pool just created.
        unsafe {
            let pool = device.raw.create_command_pool(&pool_info, None)?;
            let allocate_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(1);
            let buffer = match device.raw.allocate_command_buffers(&allocate_info) {
                Ok(buffers) => buffers[0],
                Err(err) => {
                    device.raw.destroy_command_pool(pool, None);
                    return Err(err);
                }
            };
            Ok(Self {
                device,
                pool,
                buffer,
            })
        }
    }

    /// Resets the pool and puts the command buffer into recording state.
    ///
    /// All prior work recorded from this pool must have completed.
    #[expect(unsafe_code)]
    pub fn begin(&self) -> Result<vk::CommandBuffer, vk::Result> {
        // SAFETY: The caller guarantees no submission from this pool is still in flight.
        unsafe {
            self.device
                .raw
                .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())?;
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            self.device
                .raw
                .begin_command_buffer(self.buffer, &begin_info)?;
        }
        Ok(self.buffer)
    }

    #[expect(unsafe_code)]
    pub fn end(&self) -> Result<(), vk::Result> {
        // SAFETY: The buffer is in recording state from `begin`.
        unsafe { self.device.raw.end_command_buffer(self.buffer) }
    }
}

impl Drop for CommandPool {
    #[expect(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: The decoder waits for its submissions before dropping resources.
        unsafe {
            self.device.raw.destroy_command_pool(self.pool, None);
        }
    }
}
