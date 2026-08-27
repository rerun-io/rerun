//! Runtime probe for Vulkan Video H.264 decode support.
//!
//! Everything here must fail soft: a `None` from [`probe`] makes callers fall back to
//! creating a plain device without video support.

use ash::vk;

use crate::H264DecodeCapabilities;

/// A queue on the device, identified by family and index within the family.
#[derive(Clone, Copy, Debug)]
pub struct QueueSlot {
    pub family_index: u32,
    pub queue_index: u32,
}

/// Which queues to create for video decoding, on top of wgpu's own queue.
///
/// wgpu always creates one queue in family 0 (index 0) for itself.
#[derive(Clone, Debug)]
pub struct QueuePlan {
    pub decode: QueueSlot,

    /// Queue for copying decode output into fresh output images.
    /// `None` when the copy runs on the decode queue.
    pub copy: Option<QueueSlot>,

    /// Total queue count to request per family, including wgpu's queue in family 0.
    /// Only contains families this plan puts queues in.
    pub queue_counts: Vec<(u32, u32)>,
}

/// Vulkan-level H.264 decode capabilities, needed later for session & image creation.
#[derive(Clone, Debug)]
#[expect(dead_code)] // Used from the decoder milestones on.
pub struct VulkanVideoCaps {
    /// DPB images and decode output must be one and the same image (older AMD).
    /// Otherwise we decode with distinct output images (preferred).
    pub dpb_and_output_coincide: bool,

    /// DPB slots may live in separate images. When false, all slots
    /// must be layers of one array image.
    pub separate_reference_images: bool,

    pub min_bitstream_buffer_offset_alignment: u64,
    pub min_bitstream_buffer_size_alignment: u64,

    /// Decode extents get rounded up to this granularity.
    pub picture_access_granularity: [u32; 2],
}

pub struct VulkanProbe {
    pub queue_plan: QueuePlan,
    pub capabilities: H264DecodeCapabilities,
    pub video_caps: VulkanVideoCaps,
}

/// Probes the adapter for everything H.264 decode needs.
///
/// Logs the reason at debug level whenever support is missing:
/// software rasterizers and `MoltenVK` land here, it's not an error.
#[expect(unsafe_code)] // Naked Vulkan calls on the adapter's physical device.
pub fn probe(adapter: &wgpu::Adapter) -> Option<VulkanProbe> {
    re_tracing::profile_function!();

    // The NV12 output textures need plane views on the wgpu side.
    if !adapter
        .features()
        .contains(wgpu::Features::TEXTURE_FORMAT_NV12)
    {
        re_log::debug!("No GPU video decode support: adapter lacks NV12 texture support.");
        return None;
    }

    // SAFETY: We don't destroy the returned resources, the guard is dropped at the end of this function.
    let hal_adapter = unsafe { adapter.as_hal::<wgpu::hal::api::Vulkan>() }?;
    let instance = hal_adapter.shared_instance();
    let entry = instance.entry();
    let raw_instance = instance.raw_instance();
    let physical_device = hal_adapter.raw_physical_device();

    // SAFETY: The physical device comes from this instance.
    let properties = unsafe { raw_instance.get_physical_device_properties(physical_device) };

    // Keeps the sync2 story simple (synchronization2 is core in 1.3) and every driver with
    // video decode support in practice is 1.3 anyways.
    if properties.api_version < vk::API_VERSION_1_3 {
        re_log::debug!("No GPU video decode support: Vulkan 1.3 required.");
        return None;
    }

    // SAFETY: The physical device comes from this instance.
    let extensions =
        unsafe { raw_instance.enumerate_device_extension_properties(physical_device) }.ok()?;
    for required in super::REQUIRED_EXTENSIONS {
        if !extensions
            .iter()
            .any(|extension| extension.extension_name_as_c_str() == Ok(required))
        {
            re_log::debug!("No GPU video decode support: device extension {required:?} missing.");
            return None;
        }
    }

    let queue_plan = plan_queues(raw_instance, physical_device)?;

    // H.264 decode profile to query against: High profile, progressive, 4:2:0, 8-bit.
    // Hardware H.264 decoders support Baseline/Main/High uniformly.
    let mut h264_profile = vk::VideoDecodeH264ProfileInfoKHR::default()
        .std_profile_idc(vk::native::StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_HIGH)
        .picture_layout(vk::VideoDecodeH264PictureLayoutFlagsKHR::PROGRESSIVE);
    let profile = vk::VideoProfileInfoKHR::default()
        .video_codec_operation(vk::VideoCodecOperationFlagsKHR::DECODE_H264)
        .chroma_subsampling(vk::VideoChromaSubsamplingFlagsKHR::TYPE_420)
        .luma_bit_depth(vk::VideoComponentBitDepthFlagsKHR::TYPE_8)
        .chroma_bit_depth(vk::VideoComponentBitDepthFlagsKHR::TYPE_8)
        .push_next(&mut h264_profile);

    let video_queue_instance_fns = ash::khr::video_queue::Instance::new(entry, raw_instance);

    let mut h264_capabilities = vk::VideoDecodeH264CapabilitiesKHR::default();
    let mut decode_capabilities = vk::VideoDecodeCapabilitiesKHR::default();
    let mut capabilities = vk::VideoCapabilitiesKHR::default()
        .push_next(&mut decode_capabilities)
        .push_next(&mut h264_capabilities);

    // SAFETY: All three arguments outlive the call and the structs are properly chained.
    let result = unsafe {
        (video_queue_instance_fns
            .fp()
            .get_physical_device_video_capabilities_khr)(
            physical_device,
            &raw const profile,
            &raw mut capabilities,
        )
    };
    if result != vk::Result::SUCCESS {
        re_log::debug!(
            "No GPU video decode support: H.264 video capability query failed with {result:?}."
        );
        return None;
    }

    let min_coded_extent = [
        capabilities.min_coded_extent.width,
        capabilities.min_coded_extent.height,
    ];
    let max_coded_extent = [
        capabilities.max_coded_extent.width,
        capabilities.max_coded_extent.height,
    ];
    let max_dpb_slots = capabilities.max_dpb_slots;
    let max_active_references = capabilities.max_active_reference_pictures;
    let capability_flags = capabilities.flags;
    let min_bitstream_buffer_offset_alignment = capabilities.min_bitstream_buffer_offset_alignment;
    let min_bitstream_buffer_size_alignment = capabilities.min_bitstream_buffer_size_alignment;
    let picture_access_granularity = [
        capabilities.picture_access_granularity.width,
        capabilities.picture_access_granularity.height,
    ];

    // Prefer distinct DPB & output images, fall back to coincident when that's all
    // the hardware does. The spec guarantees at least one of the two flags.
    let dpb_and_output_coincide = !decode_capabilities
        .flags
        .contains(vk::VideoDecodeCapabilityFlagsKHR::DPB_AND_OUTPUT_DISTINCT);

    // The decoded frames need to be NV12 both in the DPB and as copyable decode output.
    let output_usage =
        vk::ImageUsageFlags::VIDEO_DECODE_DST_KHR | vk::ImageUsageFlags::TRANSFER_SRC;
    let dpb_usage = if dpb_and_output_coincide {
        vk::ImageUsageFlags::VIDEO_DECODE_DPB_KHR | output_usage
    } else {
        vk::ImageUsageFlags::VIDEO_DECODE_DPB_KHR
    };
    let mut usages = vec![dpb_usage];
    if !dpb_and_output_coincide {
        usages.push(output_usage);
    }
    for usage in usages {
        if !supports_nv12_video_format(&video_queue_instance_fns, physical_device, &profile, usage)
        {
            re_log::debug!(
                "No GPU video decode support: no NV12 video format for usage {usage:?}."
            );
            return None;
        }
    }

    let video_caps = VulkanVideoCaps {
        dpb_and_output_coincide,
        separate_reference_images: capability_flags
            .contains(vk::VideoCapabilityFlagsKHR::SEPARATE_REFERENCE_IMAGES),
        min_bitstream_buffer_offset_alignment,
        min_bitstream_buffer_size_alignment,
        picture_access_granularity,
    };

    let capabilities = H264DecodeCapabilities {
        min_coded_extent,
        max_coded_extent,
        max_dpb_slots,
        max_active_references,
        max_level_idc: level_idc_number(h264_capabilities.max_level_idc),
    };

    re_log::debug!(
        "Vulkan Video H.264 decode support found: {capabilities:?}, {video_caps:?}, queue plan {queue_plan:?}."
    );

    Some(VulkanProbe {
        queue_plan,
        capabilities,
        video_caps,
    })
}

/// Finds an H.264 decode queue and a transfer-capable copy queue,
/// without touching the single queue wgpu creates for itself (family 0, index 0).
#[expect(unsafe_code)]
fn plan_queues(
    raw_instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> Option<QueuePlan> {
    struct Family {
        flags: vk::QueueFlags,
        queue_count: u32,
        video_ops: vk::VideoCodecOperationFlagsKHR,
    }

    // SAFETY: The physical device comes from this instance and `properties` is sized by the `_len` query.
    let families: Vec<Family> = unsafe {
        let count = raw_instance.get_physical_device_queue_family_properties2_len(physical_device);
        let mut video_properties = vec![vk::QueueFamilyVideoPropertiesKHR::default(); count];
        let mut properties: Vec<vk::QueueFamilyProperties2<'_>> = video_properties
            .iter_mut()
            .map(|video| vk::QueueFamilyProperties2::default().push_next(video))
            .collect();
        raw_instance.get_physical_device_queue_family_properties2(physical_device, &mut properties);

        // `properties` mutably borrows `video_properties` through the pnext chains,
        // so copy the plain fields out before reading the video properties.
        let flags_and_counts: Vec<(vk::QueueFlags, u32)> = properties
            .iter()
            .map(|properties| {
                (
                    properties.queue_family_properties.queue_flags,
                    properties.queue_family_properties.queue_count,
                )
            })
            .collect();
        drop(properties);

        flags_and_counts
            .into_iter()
            .zip(video_properties.iter())
            .map(|((flags, queue_count), video)| Family {
                flags,
                queue_count,
                video_ops: video.video_codec_operations,
            })
            .collect()
    };

    // Queues already spoken for per family: wgpu's own queue.
    let mut used_queues = vec![0_u32; families.len()];
    if let Some(first) = used_queues.first_mut() {
        *first = 1;
    }

    let mut take_queue = |family_index: usize| -> Option<QueueSlot> {
        let used = &mut used_queues[family_index];
        (*used < families[family_index].queue_count).then(|| {
            let slot = QueueSlot {
                family_index: family_index as u32,
                queue_index: *used,
            };
            *used += 1;
            slot
        })
    };

    let decode_family = families.iter().position(|family| {
        family.flags.contains(vk::QueueFlags::VIDEO_DECODE_KHR)
            && family
                .video_ops
                .contains(vk::VideoCodecOperationFlagsKHR::DECODE_H264)
    });
    let Some(decode_family) = decode_family else {
        re_log::debug!("No GPU video decode support: no H.264 decode queue family.");
        return None;
    };

    let Some(decode) = take_queue(decode_family) else {
        // Decode family is family 0 with a single queue: don't share wgpu's queue.
        re_log::debug!("No GPU video decode support: decode queue family has no queue to spare.");
        return None;
    };

    let transfer_capable = |family: &Family| {
        family.flags.intersects(
            vk::QueueFlags::TRANSFER | vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE,
        )
    };

    // Copy queue for the decode-output → display-image copy: prefer a second queue in the
    // decode family, then any other family with a queue to spare. As the last resort the
    // copy shares the decode queue (submissions on one queue are ordered anyways).
    let copy = if transfer_capable(&families[decode_family]) {
        take_queue(decode_family)
    } else {
        let copy = (0..families.len())
            .find(|&index| index != decode_family && transfer_capable(&families[index]))
            .and_then(&mut take_queue);
        if copy.is_none() {
            re_log::debug!(
                "No GPU video decode support: no transfer-capable queue for the output copy."
            );
            return None;
        }
        copy
    };

    let queue_counts = used_queues
        .iter()
        .enumerate()
        .filter_map(|(family_index, &count)| {
            // Only families we put queues in. For family 0 the count includes wgpu's queue.
            let ours = if family_index == 0 {
                count > 1
            } else {
                count > 0
            };
            ours.then_some((family_index as u32, count))
        })
        .collect();

    Some(QueuePlan {
        decode,
        copy,
        queue_counts,
    })
}

/// Whether NV12 is among the supported video formats for the given usage and profile.
#[expect(unsafe_code)]
fn supports_nv12_video_format(
    video_queue_instance_fns: &ash::khr::video_queue::Instance,
    physical_device: vk::PhysicalDevice,
    profile: &vk::VideoProfileInfoKHR<'_>,
    usage: vk::ImageUsageFlags,
) -> bool {
    let mut profile_list =
        vk::VideoProfileListInfoKHR::default().profiles(std::slice::from_ref(profile));
    let format_info = vk::PhysicalDeviceVideoFormatInfoKHR::default()
        .image_usage(usage)
        .push_next(&mut profile_list);

    let get_properties = video_queue_instance_fns
        .fp()
        .get_physical_device_video_format_properties_khr;

    // SAFETY: All arguments outlive the calls and `properties` is sized by the first call.
    unsafe {
        let mut count = 0;
        if get_properties(
            physical_device,
            &raw const format_info,
            &raw mut count,
            std::ptr::null_mut(),
        ) != vk::Result::SUCCESS
        {
            return false;
        }

        let mut properties = vec![vk::VideoFormatPropertiesKHR::default(); count as usize];
        if get_properties(
            physical_device,
            &raw const format_info,
            &raw mut count,
            properties.as_mut_ptr(),
        ) != vk::Result::SUCCESS
        {
            return false;
        }
        properties.truncate(count as usize);

        properties
            .iter()
            .any(|properties| properties.format == vk::Format::G8_B8R8_2PLANE_420_UNORM)
    }
}

/// Converts `StdVideoH264LevelIdc` (an enum counting levels from 0) to the
/// `level_idc` numbering used in bitstreams and the public API (e.g. 51 for level 5.1).
fn level_idc_number(level: vk::native::StdVideoH264LevelIdc) -> u32 {
    const LEVELS: [u32; 19] = [
        10, 11, 12, 13, 20, 21, 22, 30, 31, 32, 40, 41, 42, 50, 51, 52, 60, 61, 62,
    ];
    LEVELS.get(level as usize).copied().unwrap_or(0)
}
