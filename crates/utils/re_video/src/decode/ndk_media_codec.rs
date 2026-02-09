//! Raw FFI bindings to the Android NDK MediaCodec C API.
//!
//! These bindings cover the subset of `AMediaCodec`, `AMediaFormat`, and
//! `AMediaCodecBufferInfo` needed for synchronous video decoding.
//!
//! We hand-roll these bindings rather than depending on the `ndk-sys` crate
//! because we only need a small surface area (~20 functions) and `ndk-sys`
//! pulls in the entire NDK, which significantly increases compile times and
//! dependency weight. If the surface area grows, consider switching to `ndk-sys`.
//!
//! Reference: <https://developer.android.com/ndk/reference/group/media>

#![allow(non_camel_case_types, dead_code)]

use std::os::raw::{c_char, c_void};

// ---------------------------------------------------------------------------
// Opaque types
// ---------------------------------------------------------------------------

/// Opaque handle to an `AMediaCodec` instance.
#[repr(C)]
pub struct AMediaCodec {
    _opaque: [u8; 0],
}

/// Opaque handle to an `AMediaFormat` instance.
#[repr(C)]
pub struct AMediaFormat {
    _opaque: [u8; 0],
}

// ---------------------------------------------------------------------------
// Status / error codes
// ---------------------------------------------------------------------------

/// `media_status_t` -- result code returned by NDK media functions.
pub type media_status_t = i32;

pub const AMEDIA_OK: media_status_t = 0;
pub const AMEDIA_ERROR_UNKNOWN: media_status_t = -10000;
pub const AMEDIACODEC_INFO_TRY_AGAIN_LATER: i32 = -1;
pub const AMEDIACODEC_INFO_OUTPUT_FORMAT_CHANGED: i32 = -2;
pub const AMEDIACODEC_INFO_OUTPUT_BUFFERS_CHANGED: i32 = -3;

// ---------------------------------------------------------------------------
// Buffer info
// ---------------------------------------------------------------------------

/// Information about a decoded output buffer, filled by `AMediaCodec_dequeueOutputBuffer`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct AMediaCodecBufferInfo {
    pub offset: i32,
    pub size: i32,
    pub presentation_time_us: i64,
    pub flags: u32,
}

pub const AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM: u32 = 4;

// ---------------------------------------------------------------------------
// Extern functions -- linked from libmediandk.so
// ---------------------------------------------------------------------------

#[expect(unsafe_code, reason = "FFI bindings to NDK media C API")]
#[link(name = "mediandk")]
unsafe extern "C" {
    // ---- AMediaFormat ----

    pub fn AMediaFormat_new() -> *mut AMediaFormat;
    pub fn AMediaFormat_delete(format: *mut AMediaFormat) -> media_status_t;

    pub fn AMediaFormat_setInt32(format: *mut AMediaFormat, name: *const c_char, value: i32);

    pub fn AMediaFormat_setBuffer(
        format: *mut AMediaFormat,
        name: *const c_char,
        data: *const c_void,
        size: usize,
    );

    pub fn AMediaFormat_getInt32(
        format: *mut AMediaFormat,
        name: *const c_char,
        out: *mut i32,
    ) -> bool;

    pub fn AMediaFormat_setString(
        format: *mut AMediaFormat,
        name: *const c_char,
        value: *const c_char,
    );

    pub fn AMediaFormat_toString(format: *mut AMediaFormat) -> *const c_char;

    // ---- AMediaCodec ----

    pub fn AMediaCodec_createDecoderByType(mime_type: *const c_char) -> *mut AMediaCodec;

    pub fn AMediaCodec_configure(
        codec: *mut AMediaCodec,
        format: *const AMediaFormat,
        surface: *mut c_void, // ANativeWindow*, null for byte-buffer mode
        crypto: *mut c_void,  // AMediaCrypto*, null
        flags: u32,           // 0 for decoder
    ) -> media_status_t;

    pub fn AMediaCodec_start(codec: *mut AMediaCodec) -> media_status_t;
    pub fn AMediaCodec_stop(codec: *mut AMediaCodec) -> media_status_t;
    pub fn AMediaCodec_flush(codec: *mut AMediaCodec) -> media_status_t;
    pub fn AMediaCodec_delete(codec: *mut AMediaCodec) -> media_status_t;

    pub fn AMediaCodec_dequeueInputBuffer(
        codec: *mut AMediaCodec,
        timeout_us: i64,
    ) -> isize; // returns buffer index or negative error

    pub fn AMediaCodec_getInputBuffer(
        codec: *mut AMediaCodec,
        idx: usize,
        out_size: *mut usize,
    ) -> *mut u8;

    pub fn AMediaCodec_queueInputBuffer(
        codec: *mut AMediaCodec,
        idx: usize,
        offset: u32,
        size: usize,
        time_us: u64,
        flags: u32,
    ) -> media_status_t;

    pub fn AMediaCodec_dequeueOutputBuffer(
        codec: *mut AMediaCodec,
        info: *mut AMediaCodecBufferInfo,
        timeout_us: i64,
    ) -> isize; // returns buffer index or negative info code

    pub fn AMediaCodec_getOutputBuffer(
        codec: *mut AMediaCodec,
        idx: usize,
        out_size: *mut usize,
    ) -> *mut u8;

    pub fn AMediaCodec_releaseOutputBuffer(
        codec: *mut AMediaCodec,
        idx: usize,
        render: bool,
    ) -> media_status_t;

    pub fn AMediaCodec_getOutputFormat(codec: *mut AMediaCodec) -> *mut AMediaFormat;
}

// ---------------------------------------------------------------------------
// Well-known format key constants (C string literals)
// ---------------------------------------------------------------------------

/// `"mime"` -- MIME type key.
pub const AMEDIAFORMAT_KEY_MIME: &[u8] = b"mime\0";
/// `"width"` -- video width.
pub const AMEDIAFORMAT_KEY_WIDTH: &[u8] = b"width\0";
/// `"height"` -- video height.
pub const AMEDIAFORMAT_KEY_HEIGHT: &[u8] = b"height\0";
/// `"csd-0"` -- codec-specific data #0 (SPS for H.264, VPS+SPS+PPS for H.265).
pub const AMEDIAFORMAT_KEY_CSD0: &[u8] = b"csd-0\0";
/// `"csd-1"` -- codec-specific data #1 (PPS for H.264).
pub const AMEDIAFORMAT_KEY_CSD1: &[u8] = b"csd-1\0";
/// `"color-format"` -- output color format.
pub const AMEDIAFORMAT_KEY_COLOR_FORMAT: &[u8] = b"color-format\0";
/// `"stride"` -- output buffer stride.
pub const AMEDIAFORMAT_KEY_STRIDE: &[u8] = b"stride\0";
/// `"slice-height"` -- output buffer slice height.
pub const AMEDIAFORMAT_KEY_SLICE_HEIGHT: &[u8] = b"slice-height\0";

/// MIME type for H.264 / AVC.
pub const MIME_H264: &[u8] = b"video/avc\0";
/// MIME type for H.265 / HEVC.
pub const MIME_H265: &[u8] = b"video/hevc\0";

/// `COLOR_FormatYUV420SemiPlanar` (NV12) -- the most common output format.
pub const COLOR_FORMAT_YUV420_SEMI_PLANAR: i32 = 21;
/// `COLOR_FormatYUV420Planar` (I420).
pub const COLOR_FORMAT_YUV420_PLANAR: i32 = 19;
