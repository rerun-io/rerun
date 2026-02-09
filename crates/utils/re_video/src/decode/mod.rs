//! Video frame decoding.
//! =========================
//!
//! Whirlwind tour of how to interpret picture data (from a Video perspective)
//! ---------------------------------------------------------------------------------
//!
//! Extracted from the [av1 codec wiki](https://wiki.x266.mov/docs/colorimetry/intro) and other sources.
//! Follows the trail of information we get from our AV1 decoder.
//!
//! ### How to get from YUV to RGB?
//!
//! Things to know about the incoming yuv data:
//! * `picture.bit_depth()`
//!   * is either 8 or 16
//!   * that's how the decoder stores for us but the per component we have either 8 or 10 or 12 bits -> see `picture.bits_per_component()`
//! * `picture.pixel_layout()`
//!   * `4:0:0` grayscale
//!   * `4:2:0` half horizontal and half vertical resolution for chroma
//!   * `4:2:2` half horizontal resolution for chroma
//!   * `4:4:4` full resolution for chroma
//!   * note that the AV1 decoder gives us always (!) planar data
//! * `picture.color_range()`
//!   * yuv data range may be either `limited` or `full`
//!   * `full` is what you'd naively expect, just full use up the entire 8/10/12 bits!
//!   * `limited` means that only a certain range of values is valid
//!      * weirdly enough, DO NOT CLAMP! a lot of software may say it's limited but then use the so-called foot and head space anyways to go outside the regular colors
//!          * reportedly (read this on some forums ;-)) some players _do_ clamp, so let's not get too concerned about this
//!      * it's a remnant of the analog age, but it's still very common!
//!
//! ### Given a normalized YUV triplet, how do we get color?
//!
//! * `picture.matrix_coefficients()` (see <https://wiki.x266.mov/docs/colorimetry/matrix>)
//!   * this tells us what to multiply the incoming YUV data with to get SOME RGB data
//!   * there's various standards of how to do this, but the most common is BT.709
//!   * here's a fun special one: `identity` means it's not actually YUV, but GBR!
//! * `picture.primaries()`
//!   * now we have RGB but we kinda have no idea what that means!
//!   * the color primaries tell us which space we're in
//!   * ...meaning that if the primaries are anything else we'd have to do some conversion BUT
//!     it also means that we have no chance of displaying the picture perfectly on a screen taking in sRGB (or any other not-matching color space)
//!   * [Wikipedia says](https://en.wikipedia.org/wiki/Rec._709#Relationship_to_sRGB) sRGB uses the same primaries as BT.709
//!       * but I also found other sources (e.g. [this forum post](https://forum.doom9.org/showthread.php?p=1640342#post1640342))
//!         clamining that they're just close enough to be considered the same for practical purposes
//! * `picture.transfer_characteristics()`
//!   * until this point everything is "gamma compressed", or more accurately, went through Opto Electric Transfer Function (OETF)
//!       * i.e. measure of light in, electronic signal out
//!   * we have to keep in mind the EOTF that our screen at the other end will use which for today's renderpipeline is always sRGB
//!     (meaning it's a 2.2 gamma curve with a small linear part)
//!   * Similar to the primaries, BT.709 uses a _similar_ transfer function as sRGB, but not exactly the same
//!     <https://www.image-engineering.de/library/technotes/714-color-spaces-rec-709-vs-srgb>
//!        * There's reason to believe players just ignore this:
//!           * From a [VLC issue](https://code.videolan.org/videolan/vlc/-/issues/26999):
//!              > We do not support transfers or primaries anyway, so it does not matter
//!              > (we do support HDR transfer functions PQ and HLG, not SDR ones and we support BT.2020 primaries, but not SMPTE C (which is what BT.601 NTSC is))."
//!           * …I'm sure I found a report of other video players ignoring this and most of everything except `matrix_coefficients` but I can't find it anymore :(
//!
//! All of the above are completely optional for a video to specify and there's sometimes some interplay of relationships with those.
//! (a standard would often specify several things at once, there's typical and less typical combinations)
//! So naturally, people will use terms sloppily and interchangeably,
//! If anything is lacking a video player has to make a guess.
//! … and as discussed above, even it's there, often video players tend to ignore some settings!
//!
//! With all this out of the way…
//!
//! ### What's the state of us making use of all these things?
//!
//! * ❌ `picture.bit_depth()`
//!   * TODO(#7594): ignored, we just pretend everything is 8 bits
//! * ✅ `picture.pixel_layout()`
//! * ✅ `picture.color_range()`
//! * 🟧 `picture.matrix_coefficients()`
//!    * we try to figure out whether to use `BT.709` or `BT.601` coefficients, using other characteristics for guessing if nothing else is available.
//! * ❌ `picture.primaries()`
//! * ❌ `picture.transfer_characteristics()`
//!
//! We'll very likely be good with this until either we get specific feature requests and/or we'll start
//! supporting HDR content at which point more properties will be important!
//!

#[cfg(any(with_dav1d, with_mediacodec))]
mod async_decoder_wrapper;
#[cfg(with_dav1d)]
mod av1;

#[cfg(with_mediacodec)]
mod android_mediacodec;
#[cfg(with_mediacodec)]
mod ndk_media_codec;

#[cfg(with_ffmpeg)]
mod ffmpeg_cli;

#[cfg(with_ffmpeg)]
pub use ffmpeg_cli::FFmpegCliDecoder;
#[cfg(with_ffmpeg)]
pub use ffmpeg_cli::{
    Error as FFmpegError, FFmpegVersion, FFmpegVersionParseError, ffmpeg_download_url,
};

#[cfg(target_arch = "wasm32")]
mod webcodecs;

use crate::{SampleIndex, Time, VideoDataDescription};

#[derive(thiserror::Error, Debug, Clone)]
pub enum DecodeError {
    #[error("Unsupported codec: {0}")]
    UnsupportedCodec(String),

    #[cfg(with_dav1d)]
    #[error("dav1d: {0}")]
    Dav1d(#[from] dav1d::Error),

    #[error("To enabled native AV1 decoding, compile Rerun with the `nasm` feature enabled.")]
    Dav1dWithoutNasm,

    #[error(
        "Rerun does not yet support native AV1 decoding on Linux ARM64. See https://github.com/rerun-io/rerun/issues/7755"
    )]
    NoDav1dOnLinuxArm64,

    #[cfg(target_arch = "wasm32")]
    #[error(transparent)]
    WebDecoder(#[from] webcodecs::WebError),

    #[cfg(with_ffmpeg)]
    #[error(transparent)]
    Ffmpeg(std::sync::Arc<FFmpegError>),

    #[error("Unsupported bits per component: {0}")]
    BadBitsPerComponent(usize),
}

impl re_byte_size::SizeBytes for DecodeError {
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl DecodeError {
    pub fn should_request_more_frames(&self) -> bool {
        // Decoders often (not always!) recover from errors and will succeed eventually.
        // Gotta keep trying!
        match self {
            // Unsupported codec / decoder not available:
            Self::UnsupportedCodec(_) | Self::Dav1dWithoutNasm | Self::NoDav1dOnLinuxArm64 => false,

            // Issue with AV1 decoding.
            #[cfg(with_dav1d)]
            Self::Dav1d(_) => true,

            // Issue with WebCodecs decoding.
            #[cfg(target_arch = "wasm32")]
            Self::WebDecoder(_) => true,

            // Issue with FFmpeg decoding.
            #[cfg(with_ffmpeg)]
            Self::Ffmpeg(err) => err.should_request_more_frames(),

            // Unsupported format.
            Self::BadBitsPerComponent(_) => false,
        }
    }
}

pub type Result<T = (), E = DecodeError> = std::result::Result<T, E>;

pub type FrameResult = Result<Frame>;

/// Interface for an asynchronous video decoder.
///
/// Output callback is passed in on creation of a concrete type.
pub trait AsyncDecoder: Send + Sync {
    /// Submits a chunk for decoding in the background.
    ///
    /// Chunks are expected to come in the order of their decoding timestamp.
    fn submit_chunk(&mut self, chunk: Chunk) -> Result<()>;

    /// Called after submitting the last chunk.
    ///
    /// Should flush all pending frames.
    /// If you plan on sending more chunks after calling `end_of_video`,
    /// you MUST call [`Self::reset`] FIRST.
    ///
    /// Implementation note:
    /// As of writing there's two decoders that have requirements on what happens for new frames after `end_of_video`
    /// * WebCodec: The next submitted chunk has to be a key frame.
    /// * FFmpeg-executable: We've shut down stdin, thus we need to restart the process. Doing this without the full context of `reset` is not possible right now.
    fn end_of_video(&mut self) -> Result<()> {
        Ok(())
    }

    /// Resets the decoder.
    ///
    /// Expected to be called for backward seeking and major jumps forward in the video.
    /// Newly created decoder can assume to get reset at least once before any chunks are submitted.
    ///
    /// This does not block, all chunks sent to `decode` before this point will be discarded.
    /// Previously missing [`VideoDataDescription::encoding_details`] may be present now.
    fn reset(&mut self, video_descr: &VideoDataDescription) -> Result<()>;

    /// Minimum number of samples the decoder requests to stay head of the currently requested sample.
    ///
    /// I.e. if sample N is requested, then the encoder would like to see at least all the samples from
    /// [start of N's GOP] until [N + `min_num_samples_to_enqueue_ahead`].
    /// Codec specific constraints regarding what samples can be decoded (samples may depend on other samples in their GOP)
    /// still apply independently of this.
    ///
    /// This can be used as a workaround for decoders that are known to need additional samples to produce outputs.
    fn min_num_samples_to_enqueue_ahead(&self) -> usize {
        0
    }
}

/// Creates a new async decoder for the given `video` data.
pub fn new_decoder(
    debug_name: &str,
    video: &crate::VideoDataDescription,
    decode_settings: &DecodeSettings,
    output_sender: crate::Sender<FrameResult>,
) -> Result<Box<dyn AsyncDecoder>> {
    #![allow(clippy::allow_attributes, unused_variables, clippy::needless_return)] // With some feature flags

    re_tracing::profile_function!();

    re_log::trace!(
        "Looking for decoder for {}",
        video.human_readable_codec_string()
    );

    #[cfg(target_arch = "wasm32")]
    return Ok(Box::new(webcodecs::WebVideoDecoder::new(
        video,
        decode_settings.hw_acceleration,
        output_sender,
    )?));

    // On Android, use MediaCodec for H.264 and H.265.
    #[cfg(with_mediacodec)]
    match video.codec {
        crate::VideoCodec::H264 | crate::VideoCodec::H265 => {
            re_log::trace!("Decoding {:?} via Android MediaCodec…", video.codec);
            return Ok(Box::new(async_decoder_wrapper::AsyncDecoderWrapper::new(
                debug_name.to_owned(),
                Box::new(
                    android_mediacodec::MediaCodecDecoder::new(debug_name.to_owned(), video)
                        .map_err(|e| DecodeError::UnsupportedCodec(e.to_string()))?,
                ),
                output_sender,
            )));
        }
        _ => {} // fall through to dav1d for AV1, etc.
    }

    #[cfg(not(target_arch = "wasm32"))]
    match video.codec {
        #[cfg(feature = "av1")]
        crate::VideoCodec::AV1 => {
            #[cfg(linux_arm64)]
            {
                return Err(DecodeError::NoDav1dOnLinuxArm64);
            }

            #[cfg(with_dav1d)]
            {
                re_log::trace!("Decoding AV1…");
                return Ok(Box::new(async_decoder_wrapper::AsyncDecoderWrapper::new(
                    debug_name.to_owned(),
                    Box::new(av1::SyncDav1dDecoder::new(debug_name.to_owned())?),
                    output_sender,
                )));
            }
        }

        #[cfg(with_ffmpeg)]
        crate::VideoCodec::H264 | crate::VideoCodec::H265 => Ok(Box::new(FFmpegCliDecoder::new(
            debug_name.to_owned(),
            &video.encoding_details,
            output_sender,
            decode_settings.ffmpeg_path.clone(),
            &video.codec,
        )?)),

        _ => Err(DecodeError::UnsupportedCodec(
            video.human_readable_codec_string(),
        )),
    }
}

/// One chunk of encoded video data, representing a single [`crate::SampleMetadata`].
///
/// For details on how to interpret the data, see [`crate::SampleMetadata`].
///
/// In MP4, one sample is one frame.
pub struct Chunk {
    /// The start of a new group of pictures?
    ///
    /// This probably means this is a _keyframe_, and that and entire frame
    /// can be decoded from only this one sample (though I'm not 100% sure).
    pub is_sync: bool,

    pub data: Vec<u8>,

    /// Which sample (frame) did this chunk come from?
    ///
    /// This is the order of which the samples appear in the container,
    /// which is usually ordered by [`Self::decode_timestamp`].
    pub sample_idx: usize,

    /// Which frame does this chunk belong to?
    ///
    /// This is on the assumption that each sample produces a single frame,
    /// which is true for MP4.
    ///
    /// This is the index of samples ordered by [`Self::presentation_timestamp`].
    ///
    /// Do *not* use this to index into the video data description!
    /// Use [`Self::sample_idx`] instead.
    pub frame_nr: u32,

    /// Decode timestamp of this sample.
    /// Chunks are expected to be submitted in the order of decode timestamp.
    ///
    /// `decode_timestamp <= presentation_timestamp`
    pub decode_timestamp: Time,

    /// Time at which this sample appears in the frame stream, in time units.
    ///
    /// The frame should be shown at this time.
    /// Often synonymous with `composition_timestamp`.
    ///
    /// `decode_timestamp <= presentation_timestamp`
    pub presentation_timestamp: Time,

    /// Duration of the sample.
    ///
    /// Typically the time difference in presentation timestamp to the next sample.
    /// May be unknown if this is the last sample in an ongoing video stream.
    pub duration: Option<Time>,
}

impl re_byte_size::SizeBytes for Chunk {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            is_sync: _,
            data,
            sample_idx: _,
            frame_nr: _,
            decode_timestamp: _,
            presentation_timestamp: _,
            duration: _,
        } = self;
        data.heap_size_bytes()
    }
}

/// Data for a decoded frame on native targets.
#[cfg(not(target_arch = "wasm32"))]
pub struct FrameContent {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
}

#[cfg(not(target_arch = "wasm32"))]
impl re_byte_size::SizeBytes for FrameContent {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            data,
            width: _,
            height: _,
            format: _,
        } = self;
        data.heap_size_bytes()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl FrameContent {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

/// Data for a decoded frame on the web.
#[cfg(target_arch = "wasm32")]
pub type FrameContent = webcodecs::WebVideoFrame;

#[cfg(target_arch = "wasm32")]
impl FrameContent {
    pub fn width(&self) -> u32 {
        self.display_width()
    }

    pub fn height(&self) -> u32 {
        self.display_height()
    }
}

/// Meta information about a decoded video frame, as reported by the decoder.
#[derive(Debug, Clone)]
pub struct FrameInfo {
    /// The start of a new group of pictures?
    ///
    /// This probably means this is a _keyframe_, and that and entire frame
    /// can be decoded from only this one sample (though I'm not 100% sure).
    ///
    /// None = unknown.
    pub is_sync: Option<bool>,

    /// Which sample in the video is this from?
    ///
    /// We always assume one sample leads one frame
    /// (but may provide arbitrary additional information which may be needed for other frames in the GOP).
    ///
    /// This is the order of which the samples appear in the container,
    /// which is ordered by [`Self::latest_decode_timestamp`].
    /// I.e. this is NOT ordered by [`Self::presentation_timestamp`].
    ///
    /// None = unknown.
    pub sample_idx: Option<SampleIndex>,

    /// Which frame is this?
    ///
    /// This is on the assumption that each sample produces a single frame,
    /// which is true for MP4.
    ///
    /// This is the index of frames ordered by [`Self::presentation_timestamp`].
    ///
    /// None = unknown.
    pub frame_nr: Option<u32>,

    /// Time at which this frame appears in the frame stream, in time units.
    ///
    /// The frame should be shown at this time.
    /// We expect this timestamp to be identical with a the presentation timestamp of the [`crate::Chunk`]
    /// which is associated with this frame.
    /// Often synonymous with `composition_timestamp`.
    ///
    /// `decode_timestamp <= presentation_timestamp`
    pub presentation_timestamp: Time,

    /// Duration of the frame.
    ///
    /// Typically the time difference in presentation timestamp to the next frame.
    /// May be unknown if this is the last frame in an ongoing video stream.
    pub duration: Option<Time>,

    /// The decode timestamp of the last chunk that was needed to decode this frame.
    ///
    /// None = unknown.
    pub latest_decode_timestamp: Option<Time>,
}

impl FrameInfo {
    /// Presentation timestamp range in which this frame is valid.
    ///
    /// If there's no known duration, the range is open ended.
    pub fn presentation_time_range(&self) -> std::ops::Range<Time> {
        if let Some(duration) = self.duration {
            self.presentation_timestamp..self.presentation_timestamp + duration
        } else {
            self.presentation_timestamp..Time::MAX
        }
    }
}

/// One decoded video frame.
pub struct Frame {
    pub content: FrameContent,
    pub info: FrameInfo,
}

impl re_byte_size::SizeBytes for Frame {
    fn heap_size_bytes(&self) -> u64 {
        let Self { content, info: _ } = self;
        content.heap_size_bytes()
    }
}

/// Pixel format/layout used by [`FrameContent::data`].
#[derive(Debug, Clone)]
pub enum PixelFormat {
    Rgb8Unorm,
    Rgba8Unorm,

    Yuv {
        layout: YuvPixelLayout,
        range: YuvRange,
        // TODO(andreas): Color primaries should also apply to RGB data,
        // but for now we just always assume RGB to be BT.709 ~= sRGB.
        coefficients: YuvMatrixCoefficients,
        // Note that we don't handle chroma sample location at all so far.
    },
}

impl PixelFormat {
    pub fn bits_per_pixel(&self) -> u32 {
        match self {
            Self::Rgb8Unorm { .. } => 24,
            Self::Rgba8Unorm { .. } => 32,
            Self::Yuv { layout, .. } => match layout {
                YuvPixelLayout::Y_U_V444 => 24,
                YuvPixelLayout::Y_U_V422 => 16,
                YuvPixelLayout::Y_U_V420 => 12,
                YuvPixelLayout::Y400 => 8,
            },
        }
    }
}

/// Pixel layout used by [`PixelFormat::Yuv`].
///
/// For details see `re_renderer`'s `YuvPixelLayout` type.
#[expect(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YuvPixelLayout {
    Y_U_V444,
    Y_U_V422,
    Y_U_V420,
    Y400,
}

/// Yuv value range used by [`PixelFormat::Yuv`].
///
/// For details see `re_renderer`'s `YuvRange` type.
#[derive(Debug, Clone, Copy)]
pub enum YuvRange {
    Limited,
    Full,
}

/// Yuv matrix coefficients used by [`PixelFormat::Yuv`].
///
/// For details see `re_renderer`'s `YuvMatrixCoefficients` type.
#[derive(Debug, Clone, Copy)]
pub enum YuvMatrixCoefficients {
    /// Interpret YUV as GBR.
    Identity,

    Bt601,

    Bt709,
}

/// How the video should be decoded.
///
/// Depending on the decoder backend, these settings are merely hints and may be ignored.
/// However, they can be useful in some situations to work around issues.
///
/// On the web this directly corresponds to
/// <https://www.w3.org/TR/webcodecs/#hardware-acceleration>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum DecodeHardwareAcceleration {
    /// May use hardware acceleration if available and compatible with the codec.
    #[default]
    Auto,

    /// Should use a software decoder even if hardware acceleration is available.
    ///
    /// If no software decoder is present, this may cause decoding to fail.
    PreferSoftware,

    /// Should use a hardware decoder.
    ///
    /// If no hardware decoder is present, this may cause decoding to fail.
    PreferHardware,
}

/// Settings for video decoding.
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DecodeSettings {
    /// How the video should be decoded.
    pub hw_acceleration: DecodeHardwareAcceleration,

    /// Custom path for the ffmpeg binary.
    ///
    /// If not provided, we use the path automatically determined by `ffmpeg_sidecar`.
    #[cfg(not(target_arch = "wasm32"))]
    pub ffmpeg_path: Option<std::path::PathBuf>,
}

impl std::fmt::Display for DecodeHardwareAcceleration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "Auto"),
            Self::PreferSoftware => write!(f, "Prefer software"),
            Self::PreferHardware => write!(f, "Prefer hardware"),
        }
    }
}

impl std::str::FromStr for DecodeHardwareAcceleration {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().replace('-', "_").as_str() {
            "auto" => Ok(Self::Auto),
            "prefer_software" | "software" => Ok(Self::PreferSoftware),
            "prefer_hardware" | "hardware" => Ok(Self::PreferHardware),
            _ => Err(()),
        }
    }
}
