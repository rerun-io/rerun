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
//!   * `4:0:0` greyscale
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
//! * TODO(andreas): It may actually be non-downsampled RGB as well, so skip the entire YUV conversion step! Haven't figured out yet how to determine that.
//!
//!
//! ### Given a normalized YUV triplet, how do we get color?
//!
//! * `picture.matrix_coefficients()` (see <https://wiki.x266.mov/docs/colorimetry/matrix>)
//!   * this tells us what to multiply the incoming YUV data with to get SOME RGB data
//!   * there's various standards of how to do this, but the most common is BT.709
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
//!      <https://www.image-engineering.de/library/technotes/714-color-spaces-rec-709-vs-srgb>
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

#[cfg(feature = "av1")]
#[cfg(not(target_arch = "wasm32"))]
pub mod av1;

#[cfg(not(target_arch = "wasm32"))]
pub mod async_decoder;

#[cfg(not(target_arch = "wasm32"))]
pub use async_decoder::AsyncDecoder;

use std::sync::atomic::AtomicBool;

use crate::Time;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[cfg(feature = "av1")]
    #[cfg(not(target_arch = "wasm32"))]
    #[error("dav1d: {0}")]
    Dav1d(#[from] dav1d::Error),
}

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

pub type OutputCallback = dyn Fn(Result<Frame>) + Send + Sync;

/// Blocking decoder of video chunks.
pub trait SyncDecoder {
    /// Submit some work and read the results.
    ///
    /// Stop early if `should_stop` is `true` or turns `true`.
    fn submit_chunk(&mut self, should_stop: &AtomicBool, chunk: Chunk, on_output: &OutputCallback);

    /// Clear and reset everything
    fn reset(&mut self) {}
}

/// One chunk of encoded video data; usually one frame.
///
/// One loaded [`crate::Sample`].
pub struct Chunk {
    /// The start of a new [`crate::demux::GroupOfPictures`]?
    pub is_sync: bool,

    pub data: Vec<u8>,
    pub timestamp: Time,
    pub duration: Time,
}

/// One decoded video frame.
pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub timestamp: Time,
    pub duration: Time,
}

/// Pixel format/layout used by [`Frame::data`].
#[derive(Debug)]
pub enum PixelFormat {
    Rgb8Unorm,
    Rgba8Unorm,

    Yuv {
        layout: YuvPixelLayout,
        range: YuvRange,
        // TODO(andreas): color primaries should also apply to RGB data,
        // but for now we just always assume RGB to be BT.709 ~= sRGB.
        coefficients: YuvMatrixCoefficients,
    },
}

/// Pixel layout used by [`PixelFormat::Yuv`].
///
/// For details see `re_renderer`'s `YuvPixelLayout` type.
#[allow(non_camel_case_types)]
#[derive(Debug)]
pub enum YuvPixelLayout {
    Y_U_V444,
    Y_U_V422,
    Y_U_V420,
    Y400,
}

/// Yuv value range used by [`PixelFormat::Yuv`].
///
/// For details see `re_renderer`'s `YuvRange` type.
#[derive(Debug)]
pub enum YuvRange {
    Limited,
    Full,
}

/// Yuv matrix coefficients used by [`PixelFormat::Yuv`].
///
/// For details see `re_renderer`'s `YuvMatrixCoefficients` type.
#[derive(Debug)]
pub enum YuvMatrixCoefficients {
    Bt601,
    Bt709,
}
