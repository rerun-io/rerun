//! Raw memory allocation with dedicated-allocation RAII wrappers.
//!
//! The backend makes a handful of long-lived allocations with no fragmentation
//! pressure, so every image, buffer, and session memory block gets its own
//! `vkAllocateMemory` (which also sidesteps drivers reporting alignment 0 for
//! video session memory).

use std::sync::Arc;

use ash::vk;

use super::device::Device;

/// Picks a memory type index out of `type_bits` with all `required` property flags.
fn find_memory_type(
    properties: &vk::PhysicalDeviceMemoryProperties,
    type_bits: u32,
    required: vk::MemoryPropertyFlags,
) -> Result<u32, vk::Result> {
    (0..properties.memory_type_count)
        .find(|&index| {
            type_bits & (1 << index) != 0
                && properties.memory_types[index as usize]
                    .property_flags
                    .contains(required)
        })
        .ok_or(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY)
}

/// Allocates one dedicated memory block for the image or buffer in `dedicated_for`.
#[expect(unsafe_code)]
fn allocate(
    device: &Device,
    requirements: vk::MemoryRequirements,
    required: vk::MemoryPropertyFlags,
    dedicated_for: Option<vk::MemoryDedicatedAllocateInfo<'_>>,
) -> Result<vk::DeviceMemory, vk::Result> {
    let memory_type_index = find_memory_type(
        &device.memory_properties,
        requirements.memory_type_bits,
        required,
    )?;

    let mut allocate_info = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type_index);
    let mut dedicated = dedicated_for.unwrap_or_default();
    if dedicated_for.is_some() {
        allocate_info = allocate_info.push_next(&mut dedicated);
    }

    // SAFETY: Plain allocation, freed by the RAII wrappers.
    unsafe { device.raw.allocate_memory(&allocate_info, None) }
}

/// An image with its own dedicated memory block.
pub struct Image {
    device: Arc<Device>,
    pub raw: vk::Image,
    memory: vk::DeviceMemory,
}

impl Image {
    #[expect(unsafe_code)]
    pub fn new(
        device: Arc<Device>,
        create_info: &vk::ImageCreateInfo<'_>,
    ) -> Result<Self, vk::Result> {
        // SAFETY: Handles created here are either wrapped for RAII cleanup or
        // destroyed on the error paths.
        unsafe {
            let raw = device.raw.create_image(create_info, None)?;
            let requirements = device.raw.get_image_memory_requirements(raw);
            let dedicated = vk::MemoryDedicatedAllocateInfo::default().image(raw);
            let memory = match allocate(
                &device,
                requirements,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                Some(dedicated),
            ) {
                Ok(memory) => memory,
                Err(err) => {
                    device.raw.destroy_image(raw, None);
                    return Err(err);
                }
            };
            if let Err(err) = device.raw.bind_image_memory(raw, memory, 0) {
                device.raw.destroy_image(raw, None);
                device.raw.free_memory(memory, None);
                return Err(err);
            }
            Ok(Self {
                device,
                raw,
                memory,
            })
        }
    }
}

impl Drop for Image {
    #[expect(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: The decoder waits for its submissions before dropping resources.
        unsafe {
            self.device.raw.destroy_image(self.raw, None);
            self.device.raw.free_memory(self.memory, None);
        }
    }
}

/// A host-visible, host-coherent, persistently mapped buffer with dedicated memory.
pub struct Buffer {
    device: Arc<Device>,
    pub raw: vk::Buffer,
    pub size: u64,
    memory: vk::DeviceMemory,
    mapped: *mut u8,
}

// SAFETY: The mapped pointer refers to plain host-visible memory.
#[expect(unsafe_code)]
unsafe impl Send for Buffer {}

impl Buffer {
    #[expect(unsafe_code)]
    pub fn new_host(
        device: Arc<Device>,
        create_info: &vk::BufferCreateInfo<'_>,
    ) -> Result<Self, vk::Result> {
        // SAFETY: Handles created here are either wrapped for RAII cleanup or
        // destroyed on the error paths.
        unsafe {
            let raw = device.raw.create_buffer(create_info, None)?;
            let requirements = device.raw.get_buffer_memory_requirements(raw);
            let dedicated = vk::MemoryDedicatedAllocateInfo::default().buffer(raw);
            let required =
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            let cleanup = |memory: Option<vk::DeviceMemory>, err| {
                device.raw.destroy_buffer(raw, None);
                if let Some(memory) = memory {
                    device.raw.free_memory(memory, None);
                }
                Err(err)
            };
            let memory = match allocate(&device, requirements, required, Some(dedicated)) {
                Ok(memory) => memory,
                Err(err) => return cleanup(None, err),
            };
            if let Err(err) = device.raw.bind_buffer_memory(raw, memory, 0) {
                return cleanup(Some(memory), err);
            }
            let mapped =
                match device
                    .raw
                    .map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())
                {
                    Ok(pointer) => pointer.cast::<u8>(),
                    Err(err) => return cleanup(Some(memory), err),
                };
            Ok(Self {
                device,
                raw,
                size: create_info.size,
                memory,
                mapped,
            })
        }
    }

    /// The persistently mapped contents.
    #[expect(unsafe_code)]
    pub fn mapped_slice_mut(&mut self) -> &mut [u8] {
        // SAFETY: The whole buffer is mapped for its entire lifetime, and `&mut self`
        // means neither the host nor a recorded copy is reading it right now.
        unsafe { std::slice::from_raw_parts_mut(self.mapped, self.size as usize) }
    }

    /// The persistently mapped contents, read side.
    ///
    /// The caller must have waited for the GPU writes to complete.
    #[expect(unsafe_code)]
    pub fn mapped_slice(&self) -> &[u8] {
        // SAFETY: The whole buffer is mapped for its entire lifetime.
        unsafe { std::slice::from_raw_parts(self.mapped, self.size as usize) }
    }
}

impl Drop for Buffer {
    #[expect(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: The decoder waits for its submissions before dropping resources.
        unsafe {
            self.device.raw.destroy_buffer(self.raw, None);
            self.device.raw.free_memory(self.memory, None);
        }
    }
}

/// The dedicated memory blocks backing a video session.
pub struct SessionMemory {
    device: Arc<Device>,
    blocks: Vec<vk::DeviceMemory>,
}

impl SessionMemory {
    /// Allocates and binds memory for every requirement the session reports.
    #[expect(unsafe_code)]
    pub fn bind(device: &Arc<Device>, session: vk::VideoSessionKHR) -> Result<Self, vk::Result> {
        let get_requirements = device
            .video_queue_fns
            .fp()
            .get_video_session_memory_requirements_khr;

        // SAFETY: `requirements` is sized by the first call, and every block is
        // either bound and wrapped or freed on the error path.
        unsafe {
            let mut count = 0;
            (get_requirements(
                device.raw.handle(),
                session,
                &raw mut count,
                std::ptr::null_mut(),
            ))
            .result()?;
            let mut requirements =
                vec![vk::VideoSessionMemoryRequirementsKHR::default(); count as usize];
            (get_requirements(
                device.raw.handle(),
                session,
                &raw mut count,
                requirements.as_mut_ptr(),
            ))
            .result()?;
            requirements.truncate(count as usize);

            let mut this = Self {
                device: device.clone(),
                blocks: Vec::with_capacity(requirements.len()),
            };
            let mut binds = Vec::with_capacity(requirements.len());
            for requirement in &requirements {
                // Session memory has no dedicated-allocation handle to name, but each
                // block being its own allocation trivially satisfies the alignment.
                let memory = allocate(
                    device,
                    requirement.memory_requirements,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                    None,
                )?;
                this.blocks.push(memory);
                binds.push(
                    vk::BindVideoSessionMemoryInfoKHR::default()
                        .memory_bind_index(requirement.memory_bind_index)
                        .memory(memory)
                        .memory_offset(0)
                        .memory_size(requirement.memory_requirements.size),
                );
            }

            (device.video_queue_fns.fp().bind_video_session_memory_khr)(
                device.raw.handle(),
                session,
                binds.len() as u32,
                binds.as_ptr(),
            )
            .result()?;

            Ok(this)
        }
    }
}

impl Drop for SessionMemory {
    #[expect(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: Dropped together with (after) the session it is bound to.
        unsafe {
            for &memory in &self.blocks {
                self.device.raw.free_memory(memory, None);
            }
        }
    }
}
