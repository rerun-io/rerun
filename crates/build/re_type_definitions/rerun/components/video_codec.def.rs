// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The codec used to encode video stored in [`rerun::components::VideoSample`].
///
/// Support of these codecs by the Rerun Viewer is platform dependent.
/// For more details see check the [video reference](https://rerun.io/docs/reference/video).
#[rerun::rerun_type]
#[repr(u32)]
#[rerun(state = "unstable")]
pub enum VideoCodec {
    /// AOMedia Video 1 (AV1)
    ///
    /// See <https://en.wikipedia.org/wiki/AV1>
    ///
    /// [`rerun::components::VideoSample`]s using this codec should be formatted according the "Low overhead bitstream format",
    /// as specified in Section 5.2 of the [AV1 specification](https://aomediacodec.github.io/av1-spec/#low-overhead-bitstream-format).
    /// Each sample should be formatted as a sequence of OBUs (Open Bitstream Units) long enough to decode at least one video frame.
    /// Samples containing keyframes must include a sequence header OBU before the `KEY_FRAME` OBU to enable
    /// extraction of frame dimensions, bit depth, and color information. `INTRA_ONLY` frames are not treated
    /// as keyframes since they may reference existing decoder state.
    ///
    /// Enum value is the fourcc for 'av01' (the WebCodec string assigned to this codec) in big endian.
    AV1 = 0x61763031,

    /// Advanced Video Coding (AVC/H.264)
    ///
    /// See <https://en.wikipedia.org/wiki/Advanced_Video_Coding>
    ///
    /// [`rerun::components::VideoSample`]s using this codec should be formatted according to Annex B specification.
    /// (Note that this is different from AVCC format found in MP4 files.
    /// To learn more about Annex B, check for instance <https://membrane.stream/learn/h264/3>)
    /// Key frames (IDR) require inclusion of a SPS (Sequence Parameter Set)
    ///
    /// Enum value is the fourcc for 'avc1' (the WebCodec string assigned to this codec) in big endian.
    H264 = 0x61766331,

    /// High Efficiency Video Coding (HEVC/H.265)
    ///
    /// See <https://en.wikipedia.org/wiki/High_Efficiency_Video_Coding>
    ///
    /// [`rerun::components::VideoSample`]s using this codec should be formatted according to Annex B specification.
    /// (Note that this is different from AVCC format found in MP4 files.
    /// To learn more about Annex B, check for instance <https://membrane.stream/learn/h264/3>)
    /// Key frames (IRAP) require inclusion of a SPS (Sequence Parameter Set)
    ///
    /// Enum value is the fourcc for 'hev1' (the WebCodec string assigned to this codec) in big endian.
    H265 = 0x68657631,

    /// VP8
    ///
    /// See <https://en.wikipedia.org/wiki/VP8>
    ///
    /// Enum value is the fourcc for 'vp08' (the WebCodec string assigned to this codec) in big endian.
    VP8 = 0x76703038,

    /// VP9
    ///
    /// See <https://en.wikipedia.org/wiki/VP9>
    ///
    /// Enum value is the fourcc for 'vp09' (the WebCodec string assigned to this codec) in big endian.
    VP9 = 0x76703039,
}
