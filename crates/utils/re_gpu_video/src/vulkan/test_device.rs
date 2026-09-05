//! A Vulkan function table for CPU-only decoder tests.
//!
//! Calls record submissions and decode parameters without executing GPU work.
//! Host buffers use owned allocations, and wgpu resources use its noop backend.

#![expect(unsafe_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_void};
use std::sync::Arc;

use ash::vk;
use ash::vk::Handle as _;
use parking_lot::Mutex;

use super::Shared;
use super::caps::{QueuePlan, QueueSlot, VulkanVideoCaps};
use super::device::Device;

#[derive(Debug)]
pub struct Submission {
    pub queue: vk::Queue,
    pub waits: Vec<(vk::Semaphore, u64)>,
    pub signals: Vec<(vk::Semaphore, u64)>,
}

#[derive(Default)]
pub struct State {
    pub completed: HashMap<vk::Semaphore, u64>,
    pub query_status: i32,
    pub submissions: Vec<Submission>,
    pub session_slots: Vec<u32>,
    pub setup_slots: Vec<i32>,
    next_handle: u64,
    memory: HashMap<vk::DeviceMemory, Box<[u8]>>,
}

impl State {
    pub fn complete_submission(&mut self, index: usize) {
        for &(semaphore, value) in &self.submissions[index].signals {
            self.completed.insert(semaphore, value);
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::default();
}

pub fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with_borrow_mut(f)
}

fn handle<T: vk::Handle>() -> T {
    with_state(|state| {
        state.next_handle += 1;
        T::from_raw(state.next_handle)
    })
}

pub fn shared() -> Arc<Shared> {
    with_state(|state| *state = State::default());
    let (wgpu_device, _) = wgpu::Device::noop(&wgpu::DeviceDescriptor::default());
    // SAFETY: Each installed pointer has the Vulkan signature associated with its name.
    // The callbacks only access test-owned memory and never pass handles to a driver.
    let (instance, raw) = unsafe {
        (
            ash::Instance::load_with(function, vk::Instance::null()),
            ash::Device::load_with(function, vk::Device::null()),
        )
    };
    let device = Arc::new(Device {
        wgpu_device,
        video_queue_fns: ash::khr::video_queue::Device::new(&instance, &raw),
        video_decode_fns: ash::khr::video_decode_queue::Device::new(&instance, &raw),
        raw,
        memory_properties: vk::PhysicalDeviceMemoryProperties {
            memory_type_count: 1,
            memory_types: [vk::MemoryType {
                property_flags: vk::MemoryPropertyFlags::DEVICE_LOCAL
                    | vk::MemoryPropertyFlags::HOST_VISIBLE
                    | vk::MemoryPropertyFlags::HOST_COHERENT,
                heap_index: 0,
            }; vk::MAX_MEMORY_TYPES],
            ..Default::default()
        },
    });
    Arc::new(Shared {
        device,
        decode_queue: Mutex::new(vk::Queue::from_raw(1)),
        copy_queue: Some(Mutex::new(vk::Queue::from_raw(2))),
        queue_plan: QueuePlan {
            decode: QueueSlot {
                family_index: 1,
                queue_index: 0,
            },
            copy: Some(QueueSlot {
                family_index: 2,
                queue_index: 0,
            }),
            queue_counts: vec![(1, 1), (2, 1)],
            decode_supports_result_status: true,
        },
        capabilities: crate::H264DecodeCapabilities {
            min_coded_extent: [16, 16],
            max_coded_extent: [4096, 4096],
            max_dpb_slots: 17,
            max_active_references: 16,
            max_level_idc: 62,
        },
        video_caps: VulkanVideoCaps {
            dpb_and_output_coincide: false,
            separate_reference_images: true,
            min_bitstream_buffer_offset_alignment: 1,
            min_bitstream_buffer_size_alignment: 1,
            picture_access_granularity: [1, 1],
            std_header_version: vk::ExtensionProperties::default(),
        },
    })
}

macro_rules! create {
    ($name:ident, $info:ty, $handle:ty) => {
        unsafe extern "system" fn $name(
            _: vk::Device,
            _: *const $info,
            _: *const vk::AllocationCallbacks<'_>,
            out: *mut $handle,
        ) -> vk::Result {
            // SAFETY: The caller supplies the output pointer required by the Vulkan API.
            unsafe {
                *out = handle();
            }
            vk::Result::SUCCESS
        }
    };
}

macro_rules! empty {
    ($name:ident($($arg:ty),* $(,)?)) => {
        unsafe extern "system" fn $name($(_: $arg),*) {}
    };
}

macro_rules! success {
    ($name:ident($($arg:ty),* $(,)?)) => {
        unsafe extern "system" fn $name($(_: $arg),*) -> vk::Result { vk::Result::SUCCESS }
    };
}

create!(create_semaphore, vk::SemaphoreCreateInfo<'_>, vk::Semaphore);
create!(
    create_command_pool,
    vk::CommandPoolCreateInfo<'_>,
    vk::CommandPool
);
create!(
    create_query_pool,
    vk::QueryPoolCreateInfo<'_>,
    vk::QueryPool
);
create!(create_image, vk::ImageCreateInfo<'_>, vk::Image);
create!(
    create_image_view,
    vk::ImageViewCreateInfo<'_>,
    vk::ImageView
);
create!(create_buffer, vk::BufferCreateInfo<'_>, vk::Buffer);
create!(
    create_parameters,
    vk::VideoSessionParametersCreateInfoKHR<'_>,
    vk::VideoSessionParametersKHR
);

empty!(destroy_semaphore(vk::Device, vk::Semaphore, *const vk::AllocationCallbacks<'_>));
empty!(destroy_command_pool(vk::Device, vk::CommandPool, *const vk::AllocationCallbacks<'_>));
empty!(destroy_query_pool(vk::Device, vk::QueryPool, *const vk::AllocationCallbacks<'_>));
empty!(destroy_image(vk::Device, vk::Image, *const vk::AllocationCallbacks<'_>));
empty!(destroy_image_view(vk::Device, vk::ImageView, *const vk::AllocationCallbacks<'_>));
empty!(destroy_buffer(vk::Device, vk::Buffer, *const vk::AllocationCallbacks<'_>));
empty!(destroy_session(vk::Device, vk::VideoSessionKHR, *const vk::AllocationCallbacks<'_>));
empty!(
    destroy_parameters(
        vk::Device,
        vk::VideoSessionParametersKHR,
        *const vk::AllocationCallbacks<'_>,
    )
);
empty!(pipeline_barrier(vk::CommandBuffer, *const vk::DependencyInfo<'_>));
empty!(reset_query_pool(vk::CommandBuffer, vk::QueryPool, u32, u32));
empty!(begin_query(
    vk::CommandBuffer,
    vk::QueryPool,
    u32,
    vk::QueryControlFlags
));
empty!(end_query(vk::CommandBuffer, vk::QueryPool, u32));
empty!(begin_video(vk::CommandBuffer, *const vk::VideoBeginCodingInfoKHR<'_>));
empty!(control_video(vk::CommandBuffer, *const vk::VideoCodingControlInfoKHR<'_>));
empty!(end_video(vk::CommandBuffer, *const vk::VideoEndCodingInfoKHR<'_>));
empty!(copy_image(vk::CommandBuffer, *const vk::CopyImageInfo2<'_>));

success!(wait_semaphores(vk::Device, *const vk::SemaphoreWaitInfo<'_>, u64));
success!(reset_command_pool(
    vk::Device,
    vk::CommandPool,
    vk::CommandPoolResetFlags
));
success!(begin_command_buffer(vk::CommandBuffer, *const vk::CommandBufferBeginInfo<'_>));
success!(end_command_buffer(vk::CommandBuffer));
success!(bind_image_memory(
    vk::Device,
    vk::Image,
    vk::DeviceMemory,
    vk::DeviceSize
));
success!(bind_buffer_memory(
    vk::Device,
    vk::Buffer,
    vk::DeviceMemory,
    vk::DeviceSize
));
success!(
    bind_session_memory(
        vk::Device,
        vk::VideoSessionKHR,
        u32,
        *const vk::BindVideoSessionMemoryInfoKHR<'_>,
    )
);

unsafe extern "system" fn allocate_command_buffers(
    _: vk::Device,
    info: *const vk::CommandBufferAllocateInfo<'_>,
    out: *mut vk::CommandBuffer,
) -> vk::Result {
    // SAFETY: The caller allocates `command_buffer_count` output entries.
    unsafe {
        for index in 0..(*info).command_buffer_count {
            *out.add(index as usize) = handle();
        }
    }
    vk::Result::SUCCESS
}

unsafe extern "system" fn semaphore_counter(
    _: vk::Device,
    semaphore: vk::Semaphore,
    out: *mut u64,
) -> vk::Result {
    // SAFETY: The caller supplies writable storage for the counter.
    unsafe {
        *out = with_state(|state| state.completed.get(&semaphore).copied().unwrap_or(0));
    }
    vk::Result::SUCCESS
}

unsafe extern "system" fn submit(
    queue: vk::Queue,
    count: u32,
    infos: *const vk::SubmitInfo2<'_>,
    _: vk::Fence,
) -> vk::Result {
    // SAFETY: The caller supplies `count` submit infos with valid semaphore arrays.
    unsafe {
        for info in std::slice::from_raw_parts(infos, count as usize) {
            let read = |pointer: *const vk::SemaphoreSubmitInfo<'_>, count: u32| {
                (0..count)
                    .map(|index| {
                        let value = &*pointer.add(index as usize);
                        (value.semaphore, value.value)
                    })
                    .collect()
            };
            with_state(|state| {
                state.submissions.push(Submission {
                    queue,
                    waits: read(info.p_wait_semaphore_infos, info.wait_semaphore_info_count),
                    signals: read(
                        info.p_signal_semaphore_infos,
                        info.signal_semaphore_info_count,
                    ),
                });
            });
        }
    }
    vk::Result::SUCCESS
}

unsafe extern "system" fn create_session(
    _: vk::Device,
    info: *const vk::VideoSessionCreateInfoKHR<'_>,
    _: *const vk::AllocationCallbacks<'_>,
    out: *mut vk::VideoSessionKHR,
) -> vk::Result {
    // SAFETY: The caller supplies initialized create info and writable output storage.
    unsafe {
        with_state(|state| state.session_slots.push((*info).max_dpb_slots));
        *out = handle();
    }
    vk::Result::SUCCESS
}

unsafe extern "system" fn session_memory_requirements(
    _: vk::Device,
    _: vk::VideoSessionKHR,
    count: *mut u32,
    _: *mut vk::VideoSessionMemoryRequirementsKHR<'_>,
) -> vk::Result {
    // SAFETY: The caller supplies writable storage for the requirement count.
    unsafe {
        *count = 0;
    }
    vk::Result::SUCCESS
}

unsafe extern "system" fn query_results(
    _: vk::Device,
    _: vk::QueryPool,
    _: u32,
    count: u32,
    _: usize,
    out: *mut c_void,
    stride: vk::DeviceSize,
    _: vk::QueryResultFlags,
) -> vk::Result {
    // SAFETY: The caller supplies `count` result entries spaced by `stride` bytes.
    unsafe {
        for index in 0..count {
            *out.byte_add(index as usize * stride as usize).cast::<i32>() =
                with_state(|state| state.query_status);
        }
    }
    vk::Result::SUCCESS
}

unsafe extern "system" fn decode_video(
    _: vk::CommandBuffer,
    info: *const vk::VideoDecodeInfoKHR<'_>,
) {
    // SAFETY: The caller supplies initialized decode info and an optional setup slot.
    unsafe {
        if let Some(slot) = (*info).p_setup_reference_slot.as_ref() {
            with_state(|state| state.setup_slots.push(slot.slot_index));
        }
    }
}

unsafe extern "system" fn image_memory_requirements(
    _: vk::Device,
    _: vk::Image,
    out: *mut vk::MemoryRequirements,
) {
    // SAFETY: The caller supplies writable storage for the memory requirements.
    unsafe {
        *out = vk::MemoryRequirements {
            size: 1 << 20,
            alignment: 1,
            memory_type_bits: 1,
        };
    }
}

unsafe extern "system" fn buffer_memory_requirements(
    _: vk::Device,
    _: vk::Buffer,
    out: *mut vk::MemoryRequirements,
) {
    // SAFETY: The caller supplies writable storage for the memory requirements.
    unsafe {
        *out = vk::MemoryRequirements {
            size: 1 << 20,
            alignment: 1,
            memory_type_bits: 1,
        };
    }
}

unsafe extern "system" fn allocate_memory(
    _: vk::Device,
    info: *const vk::MemoryAllocateInfo<'_>,
    _: *const vk::AllocationCallbacks<'_>,
    out: *mut vk::DeviceMemory,
) -> vk::Result {
    let memory = handle();
    // SAFETY: The caller supplies initialized allocation info and writable output storage.
    unsafe {
        with_state(|state| {
            state.memory.insert(
                memory,
                vec![0; (*info).allocation_size as usize].into_boxed_slice(),
            )
        });
        *out = memory;
    }
    vk::Result::SUCCESS
}

unsafe extern "system" fn free_memory(
    _: vk::Device,
    memory: vk::DeviceMemory,
    _: *const vk::AllocationCallbacks<'_>,
) {
    with_state(|state| state.memory.remove(&memory));
}

unsafe extern "system" fn map_memory(
    _: vk::Device,
    memory: vk::DeviceMemory,
    offset: vk::DeviceSize,
    _: vk::DeviceSize,
    _: vk::MemoryMapFlags,
    out: *mut *mut c_void,
) -> vk::Result {
    // SAFETY: The allocation stays owned by `State` until `free_memory` is called.
    // The caller supplies a writable output pointer and an offset within the allocation.
    unsafe {
        *out = with_state(|state| {
            state
                .memory
                .get_mut(&memory)
                .unwrap()
                .as_mut_ptr()
                .add(offset as usize)
                .cast()
        });
    }
    vk::Result::SUCCESS
}

unsafe extern "system" fn get_device_proc_addr(
    _: vk::Device,
    name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    // SAFETY: The caller supplies a terminated function name. Vulkan loaders erase
    // function signatures to `PFN_vkVoidFunction` and restore them when loading tables.
    unsafe { std::mem::transmute(function(CStr::from_ptr(name))) }
}

#[expect(
    clippy::fn_to_numeric_cast_any,
    reason = "Vulkan function tables load callbacks through erased pointers"
)]
fn function(name: &CStr) -> *const c_void {
    match name.to_bytes() {
        b"vkGetDeviceProcAddr" => get_device_proc_addr as *const (),
        b"vkCreateSemaphore" => create_semaphore as *const (),
        b"vkDestroySemaphore" => destroy_semaphore as *const (),
        b"vkGetSemaphoreCounterValue" => semaphore_counter as *const (),
        b"vkWaitSemaphores" => wait_semaphores as *const (),
        b"vkQueueSubmit2" => submit as *const (),
        b"vkCreateCommandPool" => create_command_pool as *const (),
        b"vkDestroyCommandPool" => destroy_command_pool as *const (),
        b"vkAllocateCommandBuffers" => allocate_command_buffers as *const (),
        b"vkResetCommandPool" => reset_command_pool as *const (),
        b"vkBeginCommandBuffer" => begin_command_buffer as *const (),
        b"vkEndCommandBuffer" => end_command_buffer as *const (),
        b"vkCreateVideoSessionKHR" => create_session as *const (),
        b"vkDestroyVideoSessionKHR" => destroy_session as *const (),
        b"vkGetVideoSessionMemoryRequirementsKHR" => session_memory_requirements as *const (),
        b"vkBindVideoSessionMemoryKHR" => bind_session_memory as *const (),
        b"vkCreateVideoSessionParametersKHR" => create_parameters as *const (),
        b"vkDestroyVideoSessionParametersKHR" => destroy_parameters as *const (),
        b"vkCreateQueryPool" => create_query_pool as *const (),
        b"vkDestroyQueryPool" => destroy_query_pool as *const (),
        b"vkGetQueryPoolResults" => query_results as *const (),
        b"vkCreateImage" => create_image as *const (),
        b"vkDestroyImage" => destroy_image as *const (),
        b"vkCreateImageView" => create_image_view as *const (),
        b"vkDestroyImageView" => destroy_image_view as *const (),
        b"vkGetImageMemoryRequirements" => image_memory_requirements as *const (),
        b"vkCreateBuffer" => create_buffer as *const (),
        b"vkDestroyBuffer" => destroy_buffer as *const (),
        b"vkGetBufferMemoryRequirements" => buffer_memory_requirements as *const (),
        b"vkAllocateMemory" => allocate_memory as *const (),
        b"vkFreeMemory" => free_memory as *const (),
        b"vkMapMemory" => map_memory as *const (),
        b"vkBindImageMemory" => bind_image_memory as *const (),
        b"vkBindBufferMemory" => bind_buffer_memory as *const (),
        b"vkCmdPipelineBarrier2" => pipeline_barrier as *const (),
        b"vkCmdResetQueryPool" => reset_query_pool as *const (),
        b"vkCmdBeginQuery" => begin_query as *const (),
        b"vkCmdEndQuery" => end_query as *const (),
        b"vkCmdBeginVideoCodingKHR" => begin_video as *const (),
        b"vkCmdControlVideoCodingKHR" => control_video as *const (),
        b"vkCmdDecodeVideoKHR" => decode_video as *const (),
        b"vkCmdEndVideoCodingKHR" => end_video as *const (),
        b"vkCmdCopyImage2" => copy_image as *const (),
        _ => std::ptr::null(),
    }
    .cast()
}
