//! Video session and session parameters lifecycle.
//!
//! A session is bound to one codec profile, coded extent, and DPB slot count.
//! When an SPS changes any of those the decoder recreates the session (and with it
//! the DPB images). Session parameters are add-only in Vulkan, so on any parameter
//! set change the decoder recreates the parameters object from all known parameter sets.

use std::sync::Arc;

use ash::vk;

use super::alloc::SessionMemory;
use super::caps::VulkanVideoCaps;
use super::codec::CodecProfile;
use super::device::Device;

/// A video session with its bound memory and result-status query pool.
pub struct VideoSession {
    device: Arc<Device>,
    pub raw: vk::VideoSessionKHR,
    _memory: SessionMemory,

    /// Reports whether decode operations actually succeeded, when the
    /// decode queue family supports result-status queries.
    /// One query per in-flight frame slot.
    pub query_pool: Option<vk::QueryPool>,

    /// The session state must be reset with a control command before the first decode.
    pub needs_reset: bool,

    // What the session was created for, to detect when an SPS demands a new one.
    pub profile: CodecProfile,
    pub coded_extent: vk::Extent2D,
    pub max_dpb_slots: u32,
}

impl VideoSession {
    #[expect(unsafe_code)]
    pub fn new(
        device: Arc<Device>,
        video_caps: &VulkanVideoCaps,
        decode_queue_family_index: u32,
        profile: CodecProfile,
        coded_extent: vk::Extent2D,
        max_dpb_slots: u32,
        max_active_references: u32,
        result_status_query_count: u32,
    ) -> Result<Self, vk::Result> {
        re_tracing::profile_function!();

        let raw = profile.with_profile(|profile| {
            let create_info = vk::VideoSessionCreateInfoKHR::default()
                .queue_family_index(decode_queue_family_index)
                .video_profile(profile)
                .picture_format(vk::Format::G8_B8R8_2PLANE_420_UNORM)
                .max_coded_extent(coded_extent)
                .reference_picture_format(vk::Format::G8_B8R8_2PLANE_420_UNORM)
                .max_dpb_slots(max_dpb_slots)
                .max_active_reference_pictures(max_active_references)
                .std_header_version(&video_caps.std_header_version);

            let mut raw = vk::VideoSessionKHR::null();
            // SAFETY: All create-info pointers outlive the call.
            unsafe {
                (device.video_queue_fns.fp().create_video_session_khr)(
                    device.raw.handle(),
                    &raw const create_info,
                    std::ptr::null(),
                    &raw mut raw,
                )
                .result()?;
            }
            Ok(raw)
        })?;

        let destroy_session = |raw| {
            // SAFETY: Created above, nothing references it yet.
            unsafe {
                (device.video_queue_fns.fp().destroy_video_session_khr)(
                    device.raw.handle(),
                    raw,
                    std::ptr::null(),
                );
            }
        };

        let memory = match SessionMemory::bind(&device, raw) {
            Ok(memory) => memory,
            Err(err) => {
                destroy_session(raw);
                return Err(err);
            }
        };

        let query_pool = if result_status_query_count > 0 {
            let result = profile.with_profile(|profile| {
                let mut profile = *profile;
                let create_info = vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::RESULT_STATUS_ONLY_KHR)
                    .query_count(result_status_query_count)
                    .push_next(&mut profile);
                // SAFETY: Plain creation, destroyed in drop.
                unsafe { device.raw.create_query_pool(&create_info, None) }
            });
            match result {
                Ok(pool) => Some(pool),
                Err(err) => {
                    destroy_session(raw);
                    return Err(err);
                }
            }
        } else {
            None
        };

        Ok(Self {
            device,
            raw,
            _memory: memory,
            query_pool,
            needs_reset: true,
            profile,
            coded_extent,
            max_dpb_slots,
        })
    }
}

impl Drop for VideoSession {
    #[expect(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: The decoder waits for its submissions before dropping resources.
        unsafe {
            if let Some(pool) = self.query_pool {
                self.device.raw.destroy_query_pool(pool, None);
            }
            (self.device.video_queue_fns.fp().destroy_video_session_khr)(
                self.device.raw.handle(),
                self.raw,
                std::ptr::null(),
            );
        }
    }
}

/// An immutable session parameters object holding a set of SPS/PPS.
///
/// Recreated from scratch whenever a parameter set is added or changed, which is
/// simpler and just as cheap as Vulkan's add-only update path.
pub struct SessionParameters {
    device: Arc<Device>,
    pub raw: vk::VideoSessionParametersKHR,
}

impl SessionParameters {
    pub fn new_h264(
        device: Arc<Device>,
        session: &VideoSession,
        sps: &[vk::native::StdVideoH264SequenceParameterSet],
        pps: &[vk::native::StdVideoH264PictureParameterSet],
    ) -> Result<Self, vk::Result> {
        re_tracing::profile_function!();

        let add_info = vk::VideoDecodeH264SessionParametersAddInfoKHR::default()
            .std_sp_ss(sps)
            .std_pp_ss(pps);
        let mut h264_create_info = vk::VideoDecodeH264SessionParametersCreateInfoKHR::default()
            .max_std_sps_count(sps.len() as u32)
            .max_std_pps_count(pps.len() as u32)
            .parameters_add_info(&add_info);
        let create_info = vk::VideoSessionParametersCreateInfoKHR::default()
            .video_session(session.raw)
            .push_next(&mut h264_create_info);

        Self::create(device, &create_info)
    }

    pub fn new_h265(
        device: Arc<Device>,
        session: &VideoSession,
        vps: &[vk::native::StdVideoH265VideoParameterSet],
        sps: &[vk::native::StdVideoH265SequenceParameterSet],
        pps: &[vk::native::StdVideoH265PictureParameterSet],
    ) -> Result<Self, vk::Result> {
        re_tracing::profile_function!();

        let add_info = vk::VideoDecodeH265SessionParametersAddInfoKHR::default()
            .std_vp_ss(vps)
            .std_sp_ss(sps)
            .std_pp_ss(pps);
        let mut h265_create_info = vk::VideoDecodeH265SessionParametersCreateInfoKHR::default()
            .max_std_vps_count(vps.len() as u32)
            .max_std_sps_count(sps.len() as u32)
            .max_std_pps_count(pps.len() as u32)
            .parameters_add_info(&add_info);
        let create_info = vk::VideoSessionParametersCreateInfoKHR::default()
            .video_session(session.raw)
            .push_next(&mut h265_create_info);

        Self::create(device, &create_info)
    }

    #[expect(unsafe_code)]
    fn create(
        device: Arc<Device>,
        create_info: &vk::VideoSessionParametersCreateInfoKHR<'_>,
    ) -> Result<Self, vk::Result> {
        let mut raw = vk::VideoSessionParametersKHR::null();
        // SAFETY: All create-info pointers outlive the call.
        unsafe {
            (device
                .video_queue_fns
                .fp()
                .create_video_session_parameters_khr)(
                device.raw.handle(),
                &raw const *create_info,
                std::ptr::null(),
                &raw mut raw,
            )
            .result()?;
        }

        Ok(Self { device, raw })
    }
}

impl Drop for SessionParameters {
    #[expect(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: The decoder waits for its submissions before dropping resources.
        unsafe {
            (self
                .device
                .video_queue_fns
                .fp()
                .destroy_video_session_parameters_khr)(
                self.device.raw.handle(),
                self.raw,
                std::ptr::null(),
            );
        }
    }
}
