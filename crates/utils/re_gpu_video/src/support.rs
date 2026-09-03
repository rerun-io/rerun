//! What the hardware decode path accepts, checked against an H.264 SPS.

use re_video_parsing::SpsInfo;

use crate::H264DecodeCapabilities;

/// `chroma_format_idc` of 4:2:0, the only subsampling the decode profile supports.
const CHROMA_FORMAT_IDC_YUV420: u8 = 1;

/// Why the hardware decode path can't handle a stream.
#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum UnsupportedH264Stream {
    #[error("profile {profile_idc}, only Baseline, Main and High are supported")]
    Profile { profile_idc: u8 },

    #[error("interlaced video")]
    Interlaced,

    #[error("chroma subsampling other than 4:2:0")]
    ChromaFormat,

    #[error("bit depth other than 8")]
    BitDepth,

    #[error("level {level_idc}, the device supports up to {max_level_idc}")]
    Level { level_idc: u8, max_level_idc: u32 },

    #[error(
        "coded size {width}x{height} outside the supported range \
         {min_width}x{min_height} to {max_width}x{max_height}"
    )]
    CodedExtent {
        width: u32,
        height: u32,
        min_width: u32,
        min_height: u32,
        max_width: u32,
        max_height: u32,
    },

    #[error("the stream uses up to {needed} reference frames, the device supports {available}")]
    RefFrames { needed: u32, available: u32 },
}

/// Whether the stream's bitstream features are decodable, independent of the device.
pub fn h264_unsupported_bitstream(info: &SpsInfo) -> Option<UnsupportedH264Stream> {
    if !matches!(info.profile_idc, 66 | 77 | 100) {
        return Some(UnsupportedH264Stream::Profile {
            profile_idc: info.profile_idc,
        });
    }
    if !info.frames_only {
        return Some(UnsupportedH264Stream::Interlaced);
    }
    if info.chroma_format_idc != CHROMA_FORMAT_IDC_YUV420 {
        return Some(UnsupportedH264Stream::ChromaFormat);
    }
    if info.bit_depth_luma != 8 || info.bit_depth_chroma != 8 {
        return Some(UnsupportedH264Stream::BitDepth);
    }

    None
}

/// Whether the stream stays within the device's decode limits.
pub fn h264_unsupported_by_device(
    info: &SpsInfo,
    capabilities: &H264DecodeCapabilities,
) -> Option<UnsupportedH264Stream> {
    if u32::from(info.level_idc) > capabilities.max_level_idc {
        return Some(UnsupportedH264Stream::Level {
            level_idc: info.level_idc,
            max_level_idc: capabilities.max_level_idc,
        });
    }

    let [width, height] = info.coded_extent.map(u32::from);
    let [min_width, min_height] = capabilities.min_coded_extent;
    let [max_width, max_height] = capabilities.max_coded_extent;
    if width < min_width || height < min_height || width > max_width || height > max_height {
        return Some(UnsupportedH264Stream::CodedExtent {
            width,
            height,
            min_width,
            min_height,
            max_width,
            max_height,
        });
    }

    if info.max_num_ref_frames > capabilities.max_active_references {
        return Some(UnsupportedH264Stream::RefFrames {
            needed: info.max_num_ref_frames,
            available: capabilities.max_active_references,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// High profile 4:2:0 at level 4.2, well within the test device's limits.
    fn test_sps_info() -> SpsInfo {
        SpsInfo {
            profile_idc: 100,
            constraint_flags: 0,
            level_idc: 42,
            pixel_dimensions: [1920, 1080],
            coded_extent: [1920, 1088],
            chroma_format_idc: CHROMA_FORMAT_IDC_YUV420,
            bit_depth_luma: 8,
            bit_depth_chroma: 8,
            frames_only: true,
            max_num_ref_frames: 4,
            max_num_reorder_frames: 2,
        }
    }

    fn test_capabilities() -> H264DecodeCapabilities {
        H264DecodeCapabilities {
            min_coded_extent: [16, 16],
            max_coded_extent: [4096, 4096],
            max_dpb_slots: 17,
            max_active_references: 16,
            max_level_idc: 51,
        }
    }

    #[test]
    fn high_profile_4_2_0_stream_is_supported() {
        let info = test_sps_info();
        assert_eq!(h264_unsupported_bitstream(&info), None);
        assert_eq!(
            h264_unsupported_by_device(&info, &test_capabilities()),
            None
        );
    }

    #[test]
    fn chroma_subsampling_other_than_4_2_0_is_unsupported() {
        let info = SpsInfo {
            chroma_format_idc: 2,
            ..test_sps_info()
        };
        assert_eq!(
            h264_unsupported_bitstream(&info),
            Some(UnsupportedH264Stream::ChromaFormat)
        );
    }

    #[test]
    fn interlaced_video_is_unsupported() {
        let info = SpsInfo {
            frames_only: false,
            ..test_sps_info()
        };
        assert_eq!(
            h264_unsupported_bitstream(&info),
            Some(UnsupportedH264Stream::Interlaced)
        );
    }

    #[test]
    fn profiles_beyond_high_are_unsupported() {
        let info = SpsInfo {
            profile_idc: 110,
            ..test_sps_info()
        };
        assert_eq!(
            h264_unsupported_bitstream(&info),
            Some(UnsupportedH264Stream::Profile { profile_idc: 110 })
        );
    }

    #[test]
    fn level_above_the_device_maximum_is_unsupported() {
        let info = SpsInfo {
            level_idc: 52,
            ..test_sps_info()
        };
        assert_eq!(
            h264_unsupported_by_device(&info, &test_capabilities()),
            Some(UnsupportedH264Stream::Level {
                level_idc: 52,
                max_level_idc: 51,
            })
        );
    }

    /// The coded extent is compared against the device limits, not the cropped
    /// dimensions the stream displays.
    #[test]
    fn coded_extent_beyond_the_device_maximum_is_unsupported() {
        let info = SpsInfo {
            pixel_dimensions: [7680, 4320],
            coded_extent: [7680, 4320],
            ..test_sps_info()
        };
        assert!(matches!(
            h264_unsupported_by_device(&info, &test_capabilities()),
            Some(UnsupportedH264Stream::CodedExtent { .. })
        ));
    }
}
