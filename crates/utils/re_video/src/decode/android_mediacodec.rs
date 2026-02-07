//! Android MediaCodec-based video decoder for H.264 and H.265.
//!
//! Uses the NDK `AMediaCodec` C API for hardware-accelerated decoding.
//! Implements [`SyncDecoder`] so it can be wrapped by [`AsyncDecoderWrapper`]
//! for background-thread decoding, following the same pattern as the dav1d AV1 decoder.

#![expect(unsafe_code, reason = "Required for NDK MediaCodec FFI calls")]

use std::sync::atomic::{AtomicBool, Ordering};

use crate::{VideoCodec, VideoDataDescription};

use super::{
    Chunk, Frame, FrameContent, FrameInfo, FrameResult, PixelFormat, Result,
    YuvMatrixCoefficients, YuvPixelLayout, YuvRange, async_decoder_wrapper::SyncDecoder,
    ndk_media_codec as ndk,
};

use crate::Sender;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(thiserror::Error, Debug, Clone)]
pub enum MediaCodecError {
    #[error("Failed to create MediaCodec decoder for MIME type {0}")]
    CreateFailed(String),

    #[error("MediaCodec configure failed (status {0})")]
    ConfigureFailed(ndk::media_status_t),

    #[error("MediaCodec start failed (status {0})")]
    StartFailed(ndk::media_status_t),

    #[error("Unsupported video codec for MediaCodec: {0:?}")]
    UnsupportedCodec(VideoCodec),
}

// ---------------------------------------------------------------------------
// Decoder
// ---------------------------------------------------------------------------

/// Video decoder backed by Android's `AMediaCodec` hardware decoder.
///
/// Supports H.264 (AVC) and H.265 (HEVC).
///
/// # Safety
///
/// This struct holds a raw pointer to an `AMediaCodec` instance.
/// It is `Send` because `AMediaCodec` is safe to use from any thread
/// (the NDK docs specify thread safety for the synchronous API).
pub struct MediaCodecDecoder {
    codec: *mut ndk::AMediaCodec,
    debug_name: String,
    codec_type: VideoCodec,
    /// Current output dimensions (updated on format change).
    width: u32,
    height: u32,
    /// Output stride in bytes (may be > width due to alignment).
    stride: u32,
    /// Slice height (may be > height due to alignment).
    slice_height: u32,
    /// Output color format reported by the codec.
    color_format: i32,
    /// Whether the codec has been started.
    started: bool,
}

// AMediaCodec is thread-safe for the synchronous API.
unsafe impl Send for MediaCodecDecoder {}

impl MediaCodecDecoder {
    pub fn new(
        debug_name: String,
        video: &VideoDataDescription,
    ) -> std::result::Result<Self, MediaCodecError> {
        re_tracing::profile_function!();

        let mime = match video.codec {
            VideoCodec::H264 => ndk::MIME_H264.as_ptr().cast::<std::os::raw::c_char>(),
            VideoCodec::H265 => ndk::MIME_H265.as_ptr().cast::<std::os::raw::c_char>(),
            #[allow(unreachable_patterns)]
            _ => return Err(MediaCodecError::UnsupportedCodec(video.codec)),
        };

        let mime_str = match video.codec {
            VideoCodec::H264 => "video/avc",
            VideoCodec::H265 => "video/hevc",
            #[allow(unreachable_patterns)]
            _ => "unknown",
        };

        // Create the decoder
        // SAFETY: `AMediaCodec_createDecoderByType` is safe to call with a valid MIME C string.
        let codec = unsafe { ndk::AMediaCodec_createDecoderByType(mime) };
        if codec.is_null() {
            return Err(MediaCodecError::CreateFailed(mime_str.to_owned()));
        }

        // Get dimensions from encoding details or use defaults
        let (width, height) = video
            .encoding_details
            .as_ref()
            .map(|d| (d.coded_dimensions[0] as u32, d.coded_dimensions[1] as u32))
            .unwrap_or((1920, 1080));

        let mut decoder = Self {
            codec,
            debug_name,
            codec_type: video.codec,
            width,
            height,
            stride: width,
            slice_height: height,
            color_format: ndk::COLOR_FORMAT_YUV420_SEMI_PLANAR,
            started: false,
        };

        decoder.configure_and_start(video)?;

        Ok(decoder)
    }

    fn configure_and_start(
        &mut self,
        video: &VideoDataDescription,
    ) -> std::result::Result<(), MediaCodecError> {
        re_tracing::profile_function!();

        // SAFETY: `AMediaFormat_new` returns a valid format or null (which we assert).
        let format = unsafe { ndk::AMediaFormat_new() };
        assert!(!format.is_null(), "AMediaFormat_new returned null");

        // SAFETY: All calls operate on a valid `format` pointer and use null-terminated C strings.
        unsafe {
            // Set MIME type
            let mime_value = match self.codec_type {
                VideoCodec::H264 => ndk::MIME_H264.as_ptr().cast(),
                VideoCodec::H265 => ndk::MIME_H265.as_ptr().cast(),
                #[allow(unreachable_patterns)]
                _ => ndk::MIME_H264.as_ptr().cast(),
            };
            ndk::AMediaFormat_setString(
                format,
                ndk::AMEDIAFORMAT_KEY_MIME.as_ptr().cast(),
                mime_value,
            );

            // Set dimensions
            ndk::AMediaFormat_setInt32(
                format,
                ndk::AMEDIAFORMAT_KEY_WIDTH.as_ptr().cast(),
                self.width as i32,
            );
            ndk::AMediaFormat_setInt32(
                format,
                ndk::AMEDIAFORMAT_KEY_HEIGHT.as_ptr().cast(),
                self.height as i32,
            );

            // Set CSD (codec-specific data) from the encoding details if available
            if let Some(ref details) = video.encoding_details {
                if let Some(ref stsd) = details.stsd {
                    self.set_csd_from_stsd(format, stsd);
                }
            }

            // Configure the codec (null surface = byte-buffer mode)
            let status = ndk::AMediaCodec_configure(
                self.codec,
                format,
                std::ptr::null_mut(), // no surface
                std::ptr::null_mut(), // no crypto
                0,                    // decoder mode
            );
            if status != ndk::AMEDIA_OK {
                ndk::AMediaFormat_delete(format);
                return Err(MediaCodecError::ConfigureFailed(status));
            }

            // Start the codec
            let status = ndk::AMediaCodec_start(self.codec);
            if status != ndk::AMEDIA_OK {
                ndk::AMediaFormat_delete(format);
                return Err(MediaCodecError::StartFailed(status));
            }

            ndk::AMediaFormat_delete(format);
        }

        self.started = true;
        re_log::debug!(
            "MediaCodec decoder started for {} ({}x{})",
            self.debug_name,
            self.width,
            self.height,
        );

        Ok(())
    }

    /// Extract codec-specific data (SPS/PPS for H.264, VPS/SPS/PPS for H.265) from
    /// the stsd box and set as CSD on the MediaCodec format.
    ///
    /// For H.264: SPS goes into CSD-0, PPS goes into CSD-1.
    /// For H.265: VPS+SPS+PPS all go into CSD-0 (as Annex B NAL units).
    ///
    /// When data comes from the SDK via streaming (Annex B format), the parameter sets
    /// are embedded inline in the bitstream and MediaCodec will pick them up automatically.
    ///
    /// # Safety
    ///
    /// `format` must be a valid `AMediaFormat` pointer.
    unsafe fn set_csd_from_stsd(&self, format: *mut ndk::AMediaFormat, stsd: &re_mp4::StsdBox) {
        match self.codec_type {
            VideoCodec::H264 => {
                let avc1 = match &stsd.contents {
                    re_mp4::StsdBoxContent::Avc1(avc1) => avc1,
                    _ => return,
                };
                let avcc = &avc1.avcc;

                // Build CSD-0: all SPS NALUs prefixed with Annex B start codes
                let mut csd0 = Vec::new();
                for sps in &avcc.sequence_parameter_sets {
                    csd0.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
                    csd0.extend_from_slice(&sps.bytes);
                }

                // Build CSD-1: all PPS NALUs prefixed with Annex B start codes
                let mut csd1 = Vec::new();
                for pps in &avcc.picture_parameter_sets {
                    csd1.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
                    csd1.extend_from_slice(&pps.bytes);
                }

                if !csd0.is_empty() {
                    unsafe {
                        ndk::AMediaFormat_setBuffer(
                            format,
                            ndk::AMEDIAFORMAT_KEY_CSD0.as_ptr().cast(),
                            csd0.as_ptr().cast(),
                            csd0.len(),
                        );
                    }
                }
                if !csd1.is_empty() {
                    unsafe {
                        ndk::AMediaFormat_setBuffer(
                            format,
                            ndk::AMEDIAFORMAT_KEY_CSD1.as_ptr().cast(),
                            csd1.as_ptr().cast(),
                            csd1.len(),
                        );
                    }
                }
            }
            VideoCodec::H265 => {
                // For H.265 from MP4, the HVCC box contains the raw configuration.
                // We pass it as CSD-0 and let MediaCodec parse it.
                let hvcc_raw = match &stsd.contents {
                    re_mp4::StsdBoxContent::Hev1(hev1) => &hev1.hvcc.raw,
                    re_mp4::StsdBoxContent::Hvc1(hvc1) => &hvc1.hvcc.raw,
                    _ => return,
                };

                if !hvcc_raw.is_empty() {
                    unsafe {
                        ndk::AMediaFormat_setBuffer(
                            format,
                            ndk::AMEDIAFORMAT_KEY_CSD0.as_ptr().cast(),
                            hvcc_raw.as_ptr().cast(),
                            hvcc_raw.len(),
                        );
                    }
                }
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }

    /// Submit encoded data to the codec and drain any available decoded frames.
    fn decode_chunk(&mut self, chunk: &Chunk) {
        re_tracing::profile_function!();

        if !self.started {
            re_log::warn_once!("MediaCodec decoder not started, dropping chunk");
            return;
        }

        // 1. Get an input buffer and fill it
        // SAFETY: `self.codec` is a valid AMediaCodec pointer.
        let input_idx =
            unsafe { ndk::AMediaCodec_dequeueInputBuffer(self.codec, 10_000) }; // 10ms timeout

        if input_idx < 0 {
            re_log::debug!(
                "MediaCodec: no input buffer available ({}), dropping chunk",
                input_idx,
            );
            return;
        }

        let input_idx = input_idx as usize;

        // SAFETY: `self.codec` is valid and `input_idx` was returned by `dequeueInputBuffer`.
        unsafe {
            let mut buf_size: usize = 0;
            let buf_ptr = ndk::AMediaCodec_getInputBuffer(self.codec, input_idx, &mut buf_size);

            if buf_ptr.is_null() || buf_size < chunk.data.len() {
                re_log::warn_once!(
                    "MediaCodec: input buffer too small ({buf_size} < {})",
                    chunk.data.len(),
                );
                // Release the buffer without data
                ndk::AMediaCodec_queueInputBuffer(self.codec, input_idx, 0, 0, 0, 0);
                return;
            }

            std::ptr::copy_nonoverlapping(chunk.data.as_ptr(), buf_ptr, chunk.data.len());

            let pts_us = chunk.presentation_timestamp.0 as u64;
            let status = ndk::AMediaCodec_queueInputBuffer(
                self.codec,
                input_idx,
                0,
                chunk.data.len(),
                pts_us,
                0,
            );

            if status != ndk::AMEDIA_OK {
                re_log::warn_once!("MediaCodec: queueInputBuffer failed (status {status})");
            }
        }
    }

    /// Drain all available decoded output frames from the codec.
    fn drain_output(
        &mut self,
        should_stop: &AtomicBool,
        chunk: &Chunk,
        output_sender: &Sender<FrameResult>,
    ) {
        re_tracing::profile_function!();

        loop {
            if should_stop.load(Ordering::Relaxed) {
                return;
            }

            let mut info = ndk::AMediaCodecBufferInfo::default();
            // SAFETY: `self.codec` is valid.
            let output_idx = unsafe {
                ndk::AMediaCodec_dequeueOutputBuffer(self.codec, &mut info, 10_000) // 10ms timeout
            };

            if output_idx == ndk::AMEDIACODEC_INFO_TRY_AGAIN_LATER as isize {
                // No output available yet
                break;
            }

            if output_idx == ndk::AMEDIACODEC_INFO_OUTPUT_FORMAT_CHANGED as isize {
                // Output format changed -- update our cached dimensions
                self.update_output_format();
                continue;
            }

            if output_idx == ndk::AMEDIACODEC_INFO_OUTPUT_BUFFERS_CHANGED as isize {
                // Output buffers changed (deprecated, but handle gracefully)
                continue;
            }

            if output_idx < 0 {
                re_log::warn_once!("MediaCodec: dequeueOutputBuffer error ({})", output_idx);
                break;
            }

            let output_idx_usize = output_idx as usize;

            // Extract the decoded frame data
            // SAFETY: `self.codec` is valid and `output_idx_usize` was returned by `dequeueOutputBuffer`.
            let frame = unsafe {
                let mut buf_size: usize = 0;
                let buf_ptr =
                    ndk::AMediaCodec_getOutputBuffer(self.codec, output_idx_usize, &mut buf_size);

                if buf_ptr.is_null() {
                    ndk::AMediaCodec_releaseOutputBuffer(self.codec, output_idx_usize, false);
                    continue;
                }

                let data_slice = std::slice::from_raw_parts(
                    buf_ptr.add(info.offset as usize),
                    info.size as usize,
                );

                let result = self.convert_output_to_frame(data_slice, chunk);

                ndk::AMediaCodec_releaseOutputBuffer(self.codec, output_idx_usize, false);

                result
            };

            output_sender.send(frame).ok();
        }
    }

    /// Read the current output format from the codec and update cached values.
    fn update_output_format(&mut self) {
        // SAFETY: `self.codec` is valid.
        unsafe {
            let format = ndk::AMediaCodec_getOutputFormat(self.codec);
            if format.is_null() {
                return;
            }

            let mut value: i32 = 0;
            if ndk::AMediaFormat_getInt32(
                format,
                ndk::AMEDIAFORMAT_KEY_WIDTH.as_ptr().cast(),
                &mut value,
            ) {
                self.width = value as u32;
            }
            if ndk::AMediaFormat_getInt32(
                format,
                ndk::AMEDIAFORMAT_KEY_HEIGHT.as_ptr().cast(),
                &mut value,
            ) {
                self.height = value as u32;
            }
            if ndk::AMediaFormat_getInt32(
                format,
                ndk::AMEDIAFORMAT_KEY_STRIDE.as_ptr().cast(),
                &mut value,
            ) {
                self.stride = value as u32;
            }
            if ndk::AMediaFormat_getInt32(
                format,
                ndk::AMEDIAFORMAT_KEY_SLICE_HEIGHT.as_ptr().cast(),
                &mut value,
            ) {
                self.slice_height = value as u32;
            }
            if ndk::AMediaFormat_getInt32(
                format,
                ndk::AMEDIAFORMAT_KEY_COLOR_FORMAT.as_ptr().cast(),
                &mut value,
            ) {
                self.color_format = value;
            }

            re_log::debug!(
                "MediaCodec output format: {}x{} stride={} slice_height={} color_format={}",
                self.width,
                self.height,
                self.stride,
                self.slice_height,
                self.color_format,
            );

            ndk::AMediaFormat_delete(format);
        }
    }

    /// Convert a raw MediaCodec output buffer into a [`Frame`].
    ///
    /// MediaCodec typically outputs NV12 (Y plane + interleaved UV) or I420 (Y + U + V planes).
    /// We convert NV12 to planar Y_U_V420 which the Rerun renderer expects.
    fn convert_output_to_frame(&self, data: &[u8], chunk: &Chunk) -> Result<Frame> {
        re_tracing::profile_function!();

        let w = self.width as usize;
        let h = self.height as usize;
        let stride = self.stride as usize;
        let slice_height = self.slice_height as usize;

        // Use actual height if slice_height is 0 or less than height
        let effective_slice_height = if slice_height >= h { slice_height } else { h };

        let yuv_data = if self.color_format == ndk::COLOR_FORMAT_YUV420_SEMI_PLANAR {
            // NV12: Y plane followed by interleaved UV
            self.nv12_to_yuv420p(data, w, h, stride, effective_slice_height)
        } else if self.color_format == ndk::COLOR_FORMAT_YUV420_PLANAR {
            // I420: already planar Y, U, V -- just need to handle stride
            self.i420_with_stride(data, w, h, stride, effective_slice_height)
        } else {
            // Attempt to treat unknown formats as NV12 (most common on Android)
            re_log::warn_once!(
                "MediaCodec: unknown color format {}, attempting NV12 decode",
                self.color_format,
            );
            self.nv12_to_yuv420p(data, w, h, stride, effective_slice_height)
        };

        Ok(Frame {
            content: FrameContent {
                data: yuv_data,
                width: self.width,
                height: self.height,
                format: PixelFormat::Yuv {
                    layout: YuvPixelLayout::Y_U_V420,
                    range: YuvRange::Limited, // MediaCodec typically outputs limited range
                    coefficients: YuvMatrixCoefficients::Bt709,
                },
            },
            info: FrameInfo {
                is_sync: Some(chunk.is_sync),
                sample_idx: Some(chunk.sample_idx),
                frame_nr: Some(chunk.frame_nr),
                presentation_timestamp: chunk.presentation_timestamp,
                duration: chunk.duration,
                latest_decode_timestamp: Some(chunk.decode_timestamp),
            },
        })
    }

    /// Convert NV12 (Y + interleaved UV) to planar Y_U_V420 (Y + U + V).
    fn nv12_to_yuv420p(
        &self,
        data: &[u8],
        w: usize,
        h: usize,
        stride: usize,
        slice_height: usize,
    ) -> Vec<u8> {
        re_tracing::profile_function!();

        let uv_h = h / 2;
        let uv_w = w / 2;

        // Output: packed Y (w*h) + packed U (uv_w*uv_h) + packed V (uv_w*uv_h)
        let out_size = w * h + uv_w * uv_h * 2;
        let mut out = Vec::with_capacity(out_size);

        // Copy Y plane (stride-aware)
        let y_offset = 0;
        for row in 0..h {
            let src_start = y_offset + row * stride;
            let src_end = src_start + w;
            if src_end <= data.len() {
                out.extend_from_slice(&data[src_start..src_end]);
            } else {
                // Pad with zeros if data is short
                let available = data.len().saturating_sub(src_start);
                if available > 0 {
                    out.extend_from_slice(&data[src_start..src_start + available.min(w)]);
                }
                out.resize(out.len() + w - available.min(w), 0);
            }
        }

        // UV plane starts after Y plane (at stride * slice_height)
        let uv_offset = stride * slice_height;

        // De-interleave NV12 UV into separate U and V planes
        let mut u_plane = Vec::with_capacity(uv_w * uv_h);
        let mut v_plane = Vec::with_capacity(uv_w * uv_h);

        for row in 0..uv_h {
            let src_start = uv_offset + row * stride;
            for col in 0..uv_w {
                let idx = src_start + col * 2;
                if idx + 1 < data.len() {
                    u_plane.push(data[idx]);
                    v_plane.push(data[idx + 1]);
                } else {
                    u_plane.push(128);
                    v_plane.push(128);
                }
            }
        }

        out.extend_from_slice(&u_plane);
        out.extend_from_slice(&v_plane);

        out
    }

    /// Extract I420 data with stride handling.
    fn i420_with_stride(
        &self,
        data: &[u8],
        w: usize,
        h: usize,
        stride: usize,
        slice_height: usize,
    ) -> Vec<u8> {
        re_tracing::profile_function!();

        let uv_w = w / 2;
        let uv_h = h / 2;
        let uv_stride = stride / 2;

        let out_size = w * h + uv_w * uv_h * 2;
        let mut out = Vec::with_capacity(out_size);

        // Y plane
        for row in 0..h {
            let src_start = row * stride;
            let src_end = src_start + w;
            if src_end <= data.len() {
                out.extend_from_slice(&data[src_start..src_end]);
            } else {
                out.resize(out.len() + w, 0);
            }
        }

        // U plane (starts after Y at stride * slice_height)
        let u_offset = stride * slice_height;
        for row in 0..uv_h {
            let src_start = u_offset + row * uv_stride;
            let src_end = src_start + uv_w;
            if src_end <= data.len() {
                out.extend_from_slice(&data[src_start..src_end]);
            } else {
                out.resize(out.len() + uv_w, 128);
            }
        }

        // V plane (starts after U)
        let v_offset = u_offset + uv_stride * slice_height / 2;
        for row in 0..uv_h {
            let src_start = v_offset + row * uv_stride;
            let src_end = src_start + uv_w;
            if src_end <= data.len() {
                out.extend_from_slice(&data[src_start..src_end]);
            } else {
                out.resize(out.len() + uv_w, 128);
            }
        }

        out
    }

    fn stop_and_reset(&mut self) {
        if self.started {
            // SAFETY: `self.codec` is a valid `AMediaCodec` pointer.
            unsafe {
                ndk::AMediaCodec_flush(self.codec);
                ndk::AMediaCodec_stop(self.codec);
            }
            self.started = false;
        }
    }
}

impl SyncDecoder for MediaCodecDecoder {
    fn submit_chunk(
        &mut self,
        should_stop: &AtomicBool,
        chunk: Chunk,
        output_sender: &Sender<FrameResult>,
    ) {
        re_tracing::profile_function!();

        self.decode_chunk(&chunk);
        self.drain_output(should_stop, &chunk, output_sender);
    }

    fn reset(&mut self, video_data_description: &VideoDataDescription) {
        re_tracing::profile_function!();

        self.stop_and_reset();

        // Update dimensions from possibly-new encoding details
        if let Some(ref details) = video_data_description.encoding_details {
            self.width = details.coded_dimensions[0] as u32;
            self.height = details.coded_dimensions[1] as u32;
            self.stride = self.width;
            self.slice_height = self.height;
        }

        // Reconfigure and restart
        if let Err(err) = self.configure_and_start(video_data_description) {
            re_log::error!("Failed to reset MediaCodec decoder: {err}");
        }
    }
}

impl Drop for MediaCodecDecoder {
    fn drop(&mut self) {
        re_tracing::profile_function!();

        // SAFETY: `self.codec` is a valid `AMediaCodec` pointer.
        unsafe {
            if self.started {
                ndk::AMediaCodec_stop(self.codec);
            }
            ndk::AMediaCodec_delete(self.codec);
        }
    }
}
