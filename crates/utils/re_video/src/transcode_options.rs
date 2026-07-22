//! Configuration for transcoding an mp4.

use std::path::PathBuf;

use crate::VideoCodec;

/// How to transcode an mp4 stream.
///
/// The default is a no-op: a source with no requested transform is read directly, without invoking ffmpeg.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Mp4TranscodeOptions {
    /// Target output codec. `None` keeps the source codec.
    pub output_codec: Option<VideoCodec>,

    /// Keyframe interval in frames (`-g N`, with deterministic keyframe forcing).
    /// `None` uses the encoder default.
    pub gop_size: Option<u32>,

    /// Whether to use a hardware (GPU) encoder when the local ffmpeg provides one
    /// for the chosen output codec.
    pub hardware_acceleration: HwAccel,

    /// Overrides the `ffmpeg` executable used for transcoding.
    ///
    /// `None` (default) looks it up on `PATH`.
    pub ffmpeg_override: Option<PathBuf>,
}

impl Mp4TranscodeOptions {
    /// Set [`Self::output_codec`].
    #[inline]
    pub fn with_output_codec(mut self, codec: VideoCodec) -> Self {
        self.output_codec = Some(codec);
        self
    }

    /// Set [`Self::gop_size`].
    #[inline]
    pub fn with_gop_size(mut self, gop_size: u32) -> Self {
        self.gop_size = Some(gop_size);
        self
    }

    /// Set [`Self::hardware_acceleration`].
    #[inline]
    pub fn with_hardware_acceleration(mut self, hardware_acceleration: HwAccel) -> Self {
        self.hardware_acceleration = hardware_acceleration;
        self
    }

    /// Set [`Self::ffmpeg_override`].
    #[inline]
    pub fn with_ffmpeg_override(mut self, path: impl Into<PathBuf>) -> Self {
        self.ffmpeg_override = Some(path.into());
        self
    }

    /// `source_codec` is the codec of the input: requesting `output_codec` equal
    /// to it is a no-op that doesn't warrant a re-encode. Hardware acceleration
    /// alone is deliberately not a trigger either — it only influences *how* a
    /// transcode that is happening anyway is run.
    #[inline]
    pub fn requests_transform(&self, source_codec: &VideoCodec) -> bool {
        let changes_codec = self
            .output_codec
            .as_ref()
            .is_some_and(|target| target != source_codec);
        changes_codec || self.gop_size.is_some()
    }
}

/// Hardware-acceleration preference for transcoding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HwAccel {
    /// Always use a software encoder.
    #[default]
    Off,

    /// Use a hardware encoder if the local ffmpeg provides one for the target
    /// codec, otherwise fall back to a software encoder.
    ///
    /// Hardware encoders are drawn from the `NVENC` and `VideoToolbox` families
    /// only; QSV/VAAPI are not yet used. In practice that means GPU encoding is
    /// available for H.264/H.265, and for AV1 on newer NVIDIA hardware. VP8/VP9
    /// always fall back to software — their only GPU encoders are Intel QSV/VAAPI
    /// (`vp8_vaapi`/`vp9_vaapi`/`vp9_qsv`), which are out of scope; `NVENC` and
    /// `VideoToolbox` have no VP8/VP9 encoder at all.
    ///
    /// Selection is by encoder *presence* only. An encoder that is listed but
    /// non-functional at runtime (e.g. `h264_nvenc` on a machine with no NVIDIA
    /// GPU or driver) is *not* detected here.
    Auto,
}

#[cfg(test)]
mod tests {
    use super::{HwAccel, Mp4TranscodeOptions};
    use crate::VideoCodec;

    #[test]
    fn requests_transform_truth_table() {
        let h264 = VideoCodec::H264;

        // No transform requested → no re-encode.
        assert!(!Mp4TranscodeOptions::default().requests_transform(&h264));

        // Requesting the codec the source already uses is a no-op.
        assert!(
            !Mp4TranscodeOptions::default()
                .with_output_codec(VideoCodec::H264)
                .requests_transform(&h264)
        );

        // A *different* output codec is a transform.
        assert!(
            Mp4TranscodeOptions::default()
                .with_output_codec(VideoCodec::AV1)
                .requests_transform(&h264)
        );

        // A GOP size is a transform, even when the codec is unchanged.
        assert!(
            Mp4TranscodeOptions::default()
                .with_gop_size(30)
                .requests_transform(&h264)
        );
        assert!(
            Mp4TranscodeOptions::default()
                .with_output_codec(VideoCodec::H264)
                .with_gop_size(30)
                .requests_transform(&h264)
        );

        // Hardware acceleration alone never triggers a transcode.
        assert!(
            !Mp4TranscodeOptions::default()
                .with_hardware_acceleration(HwAccel::Auto)
                .requests_transform(&h264)
        );
    }
}
