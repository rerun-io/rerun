//! Runtime probe for Vulkan Video decode support, per codec.
//!
//! Everything here must fail soft: a `None` from [`probe`] makes callers fall back to
//! creating a plain device without video support.

use ash::vk;

use crate::{Codec, DecodeCapabilities};

use super::codec::CodecProfile;

/// The codecs the Vulkan backend can probe for.
pub const PROBED_CODECS: [Codec; 3] = [Codec::H264, Codec::H265, Codec::AV1];

/// The decode extension of a codec.
pub fn codec_extension(codec: Codec) -> &'static std::ffi::CStr {
    match codec {
        Codec::H264 => ash::khr::video_decode_h264::NAME,
        Codec::H265 => ash::khr::video_decode_h265::NAME,
        Codec::AV1 => ash::khr::video_decode_av1::NAME,
    }
}

/// The queue-family video operation of a codec.
fn codec_operation(codec: Codec) -> vk::VideoCodecOperationFlagsKHR {
    match codec {
        Codec::H264 => vk::VideoCodecOperationFlagsKHR::DECODE_H264,
        Codec::H265 => vk::VideoCodecOperationFlagsKHR::DECODE_H265,
        Codec::AV1 => vk::VideoCodecOperationFlagsKHR::DECODE_AV1,
    }
}

/// The profiles to probe capabilities against, most capable first.
///
/// Hardware H.264 decoders support Baseline/Main/High uniformly, and H.265
/// decoders Main, so one profile query is enough for those. AV1 makes film
/// grain part of the profile, and hardware that decodes AV1 doesn't always
/// apply grain, so both variants are probed.
fn probe_profiles(codec: Codec) -> Vec<CodecProfile> {
    match codec {
        Codec::H264 => vec![CodecProfile::H264 {
            std_profile_idc: vk::native::StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_HIGH,
        }],
        Codec::H265 => vec![CodecProfile::H265 {
            std_profile_idc: vk::native::StdVideoH265ProfileIdc_STD_VIDEO_H265_PROFILE_IDC_MAIN,
        }],
        Codec::AV1 => [true, false]
            .into_iter()
            .map(|film_grain| CodecProfile::Av1 {
                std_profile: vk::native::StdVideoAV1Profile_STD_VIDEO_AV1_PROFILE_MAIN,
                film_grain,
            })
            .collect(),
    }
}

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

    /// Queue for copying decode output into new output images.
    /// `None` when the copy runs on the decode queue.
    pub copy: Option<QueueSlot>,

    /// Total queue count to request per family, including wgpu's queue in family 0.
    /// Only contains families this plan puts queues in.
    pub queue_counts: Vec<(u32, u32)>,

    /// The decode queue family supports result-status queries,
    /// which report whether a decode operation actually succeeded.
    pub decode_supports_result_status: bool,
}

/// Vulkan-level H.264 decode capabilities, needed later for session & image creation.
#[derive(Clone, Debug)]
pub struct VulkanVideoCaps {
    /// Maximum number of decoded-picture-buffer slots.
    pub max_dpb_slots: u32,

    /// Maximum number of active reference pictures per decode operation.
    pub max_active_references: u32,

    /// DPB images and decode output must be one and the same image (older AMD).
    /// Otherwise we decode with distinct output images (preferred).
    pub dpb_and_output_coincide: bool,

    /// DPB slots may live in separate images. When false, all slots
    /// must be layers of one array image (which the backend always uses).
    #[expect(dead_code, reason = "recorded for driver-quirk handling later")]
    pub separate_reference_images: bool,

    /// The backend always decodes from buffer offset 0, trivially aligned.
    #[expect(dead_code, reason = "recorded for driver-quirk handling later")]
    pub min_bitstream_buffer_offset_alignment: u64,

    pub min_bitstream_buffer_size_alignment: u64,

    /// Decode extents get rounded up to this granularity.
    #[expect(dead_code, reason = "recorded for driver-quirk handling later")]
    pub picture_access_granularity: [u32; 2],

    /// AV1 only: the decoder can apply the film grain a stream carries.
    /// Streams that apply grain fall back to software decoding without it.
    pub av1_film_grain_support: bool,

    /// The H.264 decode std header version the driver supports,
    /// passed back on video session creation.
    pub std_header_version: vk::ExtensionProperties,
}

/// A device's support for decoding one codec.
#[derive(Clone, Debug)]
pub struct CodecSupport {
    /// The public half.
    pub capabilities: DecodeCapabilities,

    /// The Vulkan half, needed for session & image creation.
    pub video_caps: VulkanVideoCaps,
}

/// The probed decode support per codec.
#[derive(Clone, Debug, Default)]
pub struct SupportedCodecs {
    pub h264: Option<CodecSupport>,
    pub h265: Option<CodecSupport>,
    pub av1: Option<CodecSupport>,
}

impl SupportedCodecs {
    pub fn get(&self, codec: Codec) -> Option<&CodecSupport> {
        match codec {
            Codec::H264 => self.h264.as_ref(),
            Codec::H265 => self.h265.as_ref(),
            Codec::AV1 => self.av1.as_ref(),
        }
    }

    fn set(&mut self, codec: Codec, support: CodecSupport) {
        match codec {
            Codec::H264 => self.h264 = Some(support),
            Codec::H265 => self.h265 = Some(support),
            Codec::AV1 => self.av1 = Some(support),
        }
    }

    fn unset(&mut self, codec: Codec) {
        match codec {
            Codec::H264 => self.h264 = None,
            Codec::H265 => self.h265 = None,
            Codec::AV1 => self.av1 = None,
        }
    }

    /// The supported codecs.
    pub fn codecs(&self) -> impl Iterator<Item = Codec> + '_ {
        PROBED_CODECS
            .into_iter()
            .filter(|&codec| self.get(codec).is_some())
    }
}

pub struct VulkanProbe {
    pub queue_plan: QueuePlan,
    pub codecs: SupportedCodecs,
}

/// Probes the adapter for everything video decoding needs, per codec.
///
/// Returns `None` when no codec is supported at all. Logs the reason at debug
/// level whenever support is missing: software rasterizers and `MoltenVK` land
/// here, it's not an error.
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
    let has_extension = |name: &std::ffi::CStr| {
        extensions
            .iter()
            .any(|extension| extension.extension_name_as_c_str() == Ok(name))
    };
    for required in super::BASE_EXTENSIONS {
        if !has_extension(required) {
            re_log::debug!("No GPU video decode support: device extension {required:?} missing.");
            return None;
        }
    }

    let video_queue_instance_fns = ash::khr::video_queue::Instance::new(entry, raw_instance);

    let mut codecs = SupportedCodecs::default();
    let mut wanted_ops = vk::VideoCodecOperationFlagsKHR::empty();
    for codec in PROBED_CODECS {
        if !has_extension(codec_extension(codec)) {
            re_log::debug!(
                "No GPU {codec} decode support: device extension {:?} missing.",
                codec_extension(codec)
            );
            continue;
        }
        if let Some(support) = probe_codec(&video_queue_instance_fns, physical_device, codec) {
            codecs.set(codec, support);
            wanted_ops |= codec_operation(codec);
        }
    }
    if wanted_ops.is_empty() {
        re_log::debug!("No GPU video decode support: no supported codec.");
        return None;
    }

    let (queue_plan, family_ops) = plan_queues(raw_instance, physical_device, wanted_ops)?;

    // Drop codecs the chosen decode queue family can't decode.
    for codec in PROBED_CODECS {
        if codecs.get(codec).is_some() && !family_ops.contains(codec_operation(codec)) {
            re_log::debug!("No GPU {codec} decode support: the decode queue family lacks it.");
            codecs.unset(codec);
        }
    }
    codecs.codecs().next()?;

    re_log::debug!("Vulkan Video decode support found: {codecs:?}, queue plan {queue_plan:?}.");

    Some(VulkanProbe { queue_plan, codecs })
}

/// Queries the capabilities and NV12 format support of a codec, trying its probe
/// profiles in order of preference.
fn probe_codec(
    video_queue_instance_fns: &ash::khr::video_queue::Instance,
    physical_device: vk::PhysicalDevice,
    codec: Codec,
) -> Option<CodecSupport> {
    probe_profiles(codec).into_iter().find_map(|profile| {
        probe_profile(video_queue_instance_fns, physical_device, codec, profile)
    })
}

/// Queries the capabilities and NV12 format support of one codec profile.
#[expect(unsafe_code)]
fn probe_profile(
    video_queue_instance_fns: &ash::khr::video_queue::Instance,
    physical_device: vk::PhysicalDevice,
    codec: Codec,
    probe_profile: CodecProfile,
) -> Option<CodecSupport> {
    probe_profile.with_profile(|profile| {
        let mut h264_capabilities = vk::VideoDecodeH264CapabilitiesKHR::default();
        let mut h265_capabilities = vk::VideoDecodeH265CapabilitiesKHR::default();
        let mut av1_capabilities = vk::VideoDecodeAV1CapabilitiesKHR::default();
        let mut decode_capabilities = vk::VideoDecodeCapabilitiesKHR::default();
        let mut capabilities =
            vk::VideoCapabilitiesKHR::default().push_next(&mut decode_capabilities);
        capabilities = match codec {
            Codec::H264 => capabilities.push_next(&mut h264_capabilities),
            Codec::H265 => capabilities.push_next(&mut h265_capabilities),
            Codec::AV1 => capabilities.push_next(&mut av1_capabilities),
        };

        // SAFETY: All three arguments outlive the call and the structs are properly chained.
        let result = unsafe {
            (video_queue_instance_fns
                .fp()
                .get_physical_device_video_capabilities_khr)(
                physical_device,
                &raw const *profile,
                &raw mut capabilities,
            )
        };
        if result != vk::Result::SUCCESS {
            re_log::debug!(
                "No GPU {codec} decode support: video capability query failed with {result:?}."
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
        let min_bitstream_buffer_offset_alignment =
            capabilities.min_bitstream_buffer_offset_alignment;
        let min_bitstream_buffer_size_alignment = capabilities.min_bitstream_buffer_size_alignment;
        let picture_access_granularity = [
            capabilities.picture_access_granularity.width,
            capabilities.picture_access_granularity.height,
        ];
        let std_header_version = capabilities.std_header_version;

        // Prefer distinct DPB & output images, fall back to coincident when that's all
        // the hardware does. The spec guarantees at least one of the two flags.
        let dpb_and_output_coincide = !decode_capabilities
            .flags
            .contains(vk::VideoDecodeCapabilityFlagsKHR::DPB_AND_OUTPUT_DISTINCT);

        let max_level_idc = match codec {
            Codec::H264 => h264_level_idc_number(h264_capabilities.max_level_idc),
            Codec::H265 => h265_level_idc_number(h265_capabilities.max_level_idc),
            Codec::AV1 => av1_level_number(av1_capabilities.max_level),
        };

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
            if !supports_nv12_video_format(
                video_queue_instance_fns,
                physical_device,
                profile,
                usage,
            ) {
                re_log::debug!(
                    "No GPU {codec} decode support: no NV12 video format for usage {usage:?}."
                );
                return None;
            }
        }

        Some(CodecSupport {
            capabilities: DecodeCapabilities {
                min_coded_extent: Some(min_coded_extent),
                max_coded_extent: Some(max_coded_extent),
                max_level_idc: Some(max_level_idc),
                hardware_accelerated: true,
            },
            video_caps: VulkanVideoCaps {
                max_dpb_slots,
                max_active_references,
                dpb_and_output_coincide,
                separate_reference_images: capability_flags
                    .contains(vk::VideoCapabilityFlagsKHR::SEPARATE_REFERENCE_IMAGES),
                min_bitstream_buffer_offset_alignment,
                min_bitstream_buffer_size_alignment,
                picture_access_granularity,
                av1_film_grain_support: matches!(
                    probe_profile,
                    CodecProfile::Av1 {
                        film_grain: true,
                        ..
                    }
                ),
                std_header_version,
            },
        })
    })
}

/// Finds a decode queue for the wanted codec operations and a transfer-capable
/// copy queue, without touching the single queue wgpu creates for itself
/// (family 0, index 0).
///
/// Also returns the video operations of the chosen decode family: codecs whose
/// operation it lacks aren't decodable with this plan.
#[expect(unsafe_code)]
fn plan_queues(
    raw_instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    wanted_ops: vk::VideoCodecOperationFlagsKHR,
) -> Option<(QueuePlan, vk::VideoCodecOperationFlagsKHR)> {
    struct Family {
        flags: vk::QueueFlags,
        queue_count: u32,
        video_ops: vk::VideoCodecOperationFlagsKHR,
        result_status: bool,
    }

    // SAFETY: The physical device comes from this instance and `properties` is sized by the `_len` query.
    let families: Vec<Family> = unsafe {
        let count = raw_instance.get_physical_device_queue_family_properties2_len(physical_device);
        let mut video_properties = vec![vk::QueueFamilyVideoPropertiesKHR::default(); count];
        let mut query_properties =
            vec![vk::QueueFamilyQueryResultStatusPropertiesKHR::default(); count];
        let mut properties: Vec<vk::QueueFamilyProperties2<'_>> = video_properties
            .iter_mut()
            .zip(query_properties.iter_mut())
            .map(|(video, query)| {
                vk::QueueFamilyProperties2::default()
                    .push_next(video)
                    .push_next(query)
            })
            .collect();
        raw_instance.get_physical_device_queue_family_properties2(physical_device, &mut properties);

        // `properties` mutably borrows the chained property structs,
        // so copy the plain fields out before reading them.
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
            .zip(video_properties.iter().zip(query_properties.iter()))
            .map(|((flags, queue_count), (video, query))| Family {
                flags,
                queue_count,
                video_ops: video.video_codec_operations,
                result_status: query.query_result_status_support != vk::FALSE,
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

    // The family covering the most wanted codec operations, ties broken by index.
    let decode_family = families
        .iter()
        .enumerate()
        .filter(|(_, family)| {
            family.flags.contains(vk::QueueFlags::VIDEO_DECODE_KHR)
                && family.video_ops.intersects(wanted_ops)
        })
        .max_by_key(|(index, family)| {
            (
                (family.video_ops & wanted_ops).as_raw().count_ones(),
                std::cmp::Reverse(*index),
            )
        })
        .map(|(index, _)| index);
    let Some(decode_family) = decode_family else {
        re_log::debug!("No GPU video decode support: no decode queue family for {wanted_ops:?}.");
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
            .filter(|&index| index != decode_family && transfer_capable(&families[index]))
            .find_map(&mut take_queue);
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

    Some((
        QueuePlan {
            decode,
            copy,
            queue_counts,
            decode_supports_result_status: families[decode_family].result_status,
        },
        families[decode_family].video_ops,
    ))
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
fn h264_level_idc_number(level: vk::native::StdVideoH264LevelIdc) -> u32 {
    const LEVELS: [u32; 19] = [
        10, 11, 12, 13, 20, 21, 22, 30, 31, 32, 40, 41, 42, 50, 51, 52, 60, 61, 62,
    ];
    LEVELS.get(level as usize).copied().unwrap_or(0)
}

/// Converts `StdVideoH265LevelIdc` (an enum counting levels from 0) to the same
/// level numbering the public API reports for H.264 (e.g. 51 for level 5.1).
fn h265_level_idc_number(level: vk::native::StdVideoH265LevelIdc) -> u32 {
    const LEVELS: [u32; 13] = [10, 20, 21, 30, 31, 40, 41, 50, 51, 52, 60, 61, 62];
    LEVELS.get(level as usize).copied().unwrap_or(0)
}

/// Converts `StdVideoAV1Level` (four minor levels per major level, counted from
/// 2.0) to the same level numbering the public API reports for the other codecs
/// (e.g. 51 for level 5.1).
fn av1_level_number(level: vk::native::StdVideoAV1Level) -> u32 {
    /// Levels 2.0 through 7.3.
    const LEVEL_COUNT: u32 = 24;

    if level >= LEVEL_COUNT {
        return 0;
    }
    (2 + level / 4) * 10 + level % 4
}
