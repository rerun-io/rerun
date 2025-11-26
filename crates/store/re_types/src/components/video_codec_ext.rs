#[cfg(feature = "video")]
impl TryFrom<re_video::VideoCodec> for crate::components::VideoCodec {
    type Error = String;

    fn try_from(value: re_video::VideoCodec) -> Result<Self, Self::Error> {
        match value {
            re_video::VideoCodec::H264 => Ok(Self::H264),
            re_video::VideoCodec::H265 => Ok(Self::H265),
            re_video::VideoCodec::AV1 => Ok(Self::AV1),
            // TODO(#10186): Add support for VP9.
            re_video::VideoCodec::VP8 | re_video::VideoCodec::VP9 => Err(format!(
                "Video codec {value:?} is not supported for VideoStream yet",
            )),
        }
    }
}

#[cfg(feature = "video")]
impl From<crate::components::VideoCodec> for re_video::VideoCodec {
    fn from(val: crate::components::VideoCodec) -> Self {
        match val {
            crate::components::VideoCodec::H264 => Self::H264,
            crate::components::VideoCodec::H265 => Self::H265,
            crate::components::VideoCodec::AV1 => Self::AV1,
            // TODO(#10186): Add support for VP9.
            // VideoCodec::VP8 => Self::VP8,
            // VideoCodec::VP9 => Self::VP9,
        }
    }
}
