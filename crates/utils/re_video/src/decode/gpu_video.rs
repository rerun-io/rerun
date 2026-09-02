//! H.264 decoding on the GPU via `re_gpu_video`, producing frames that never leave VRAM.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use re_gpu_video::{GpuVideoContext, H264DecodeCapabilities};

use super::sync_decoder_wrapper::{SyncDecoder, SyncDecoderWrapper};
use super::{AsyncDecoder, Chunk, DecodeError, Frame, FrameContent, FrameInfo, FrameResult};
use crate::h264::write_avc_chunk_to_nalu_stream;
use crate::nalu::AnnexBStreamState;
use crate::{
    ChromaSubsamplingModes, FrameNumber, Sender, Time, VideoDataDescription, VideoEncodingDetails,
    VideoSource,
};

/// Why the GPU decoder can't handle a stream, judging from the encoding details of the demuxer.
///
/// `None` means the stream is supported, or the details don't say enough to rule it out.
/// The GPU decoder does its own check of the stream's SPS when it starts decoding.
pub fn h264_unsupported_reason(
    encoding_details: Option<&VideoEncodingDetails>,
    capabilities: &H264DecodeCapabilities,
) -> Option<String> {
    let details = encoding_details?;

    if let Some(chroma_subsampling) = details.chroma_subsampling
        && chroma_subsampling != ChromaSubsamplingModes::Yuv420
    {
        return Some(format!(
            "chroma subsampling {chroma_subsampling} instead of 4:2:0"
        ));
    }

    if let Some(bit_depth) = details.bit_depth
        && bit_depth != 8
    {
        return Some(format!("{bit_depth} bits per color component instead of 8"));
    }

    if details.frames_only == Some(false) {
        return Some("interlaced video".to_owned());
    }

    if let Some((profile_idc, level_idc)) = h264_profile_and_level(&details.codec_string) {
        if !matches!(profile_idc, 66 | 77 | 100) {
            return Some(format!(
                "profile {profile_idc}, only Baseline, Main and High are supported"
            ));
        }
        if u32::from(level_idc) > capabilities.max_level_idc {
            return Some(format!(
                "level {level_idc}, the device supports up to {}",
                capabilities.max_level_idc
            ));
        }
    }

    let [width, height] = details.coded_dimensions.map(u32::from);
    let [min_width, min_height] = capabilities.min_coded_extent;
    let [max_width, max_height] = capabilities.max_coded_extent;
    if width < min_width || height < min_height || width > max_width || height > max_height {
        return Some(format!(
            "coded size {width}x{height} outside the supported range {min_width}x{min_height} to {max_width}x{max_height}"
        ));
    }

    None
}

/// `profile_idc` and `level_idc` of an `avc1.PPCCLL` codec string, where each pair is hex.
///
/// `None` for any string that doesn't have that shape.
fn h264_profile_and_level(codec_string: &str) -> Option<(u8, u8)> {
    let digits = codec_string.strip_prefix("avc1.")?;
    if digits.len() != 6 {
        return None;
    }
    let profile_idc = u8::from_str_radix(digits.get(0..2)?, 16).ok()?;
    let level_idc = u8::from_str_radix(digits.get(4..6)?, 16).ok()?;
    Some((profile_idc, level_idc))
}

/// Decodes H.264 to GPU textures using the [`re_gpu_video`] backend of the render device.
///
/// The backend work runs on a dedicated decoder thread via [`SyncDecoderWrapper`],
/// frames cross the output channel as [`FrameContent::GpuTexture`].
pub struct GpuDecoder {
    wrapper: SyncDecoderWrapper,

    /// `max_num_reorder_frames` of the stream's active SPS, updated by the decoder thread.
    reorder_delay: Arc<AtomicUsize>,
}

impl GpuDecoder {
    pub fn new(
        debug_name: String,
        context: &GpuVideoContext,
        video_descr: &VideoDataDescription,
        output_sender: Sender<FrameResult>,
    ) -> Result<Self, re_gpu_video::DecodeError> {
        let decoder = context.create_h264_decoder()?;
        let reorder_delay = Arc::new(AtomicUsize::new(0));

        let sync_decoder = GpuSyncDecoder {
            decoder,
            input_format: InputFormat::from_descr(video_descr),
            annexb_buffer: Vec::new(),
            pending_frame_infos: BTreeMap::new(),
            reorder_delay: reorder_delay.clone(),
        };

        Ok(Self {
            wrapper: SyncDecoderWrapper::new(debug_name, Box::new(sync_decoder), output_sender),
            reorder_delay,
        })
    }
}

impl AsyncDecoder for GpuDecoder {
    fn submit_chunk(&mut self, chunk: Chunk) -> super::Result<()> {
        self.wrapper.submit_chunk(chunk)
    }

    fn end_of_video(&mut self) -> super::Result<()> {
        self.wrapper.end_of_video()
    }

    fn reset(&mut self, video_descr: &VideoDataDescription) -> super::Result<()> {
        self.wrapper.reset(video_descr)
    }

    fn min_num_samples_to_enqueue_ahead(&self) -> usize {
        self.reorder_delay.load(Ordering::Relaxed)
    }
}

/// What conversion `Chunk::data` needs before it can be pushed as an annex-b access unit.
enum InputFormat {
    /// Streamed H.264 is already annex-b, pass it through verbatim.
    AnnexB,

    /// H.264 in MP4: prepend the SPS/PPS to each IDR frame and turn the
    /// length-prefixed NALs into annex-b.
    Avcc {
        avcc: re_mp4::Avc1Box,
        state: AnnexBStreamState,
    },
}

impl InputFormat {
    fn from_descr(video_descr: &VideoDataDescription) -> Self {
        match video_descr
            .encoding_details
            .as_ref()
            .and_then(|details| details.stsd.as_ref())
            .map(|stsd| &stsd.contents)
        {
            Some(re_mp4::StsdBoxContent::Avc1(avc1)) => Self::Avcc {
                avcc: avc1.clone(),
                state: AnnexBStreamState::default(),
            },
            _ => Self::AnnexB,
        }
    }
}

/// Frame metadata remembered per submitted chunk until its decoded frame comes out.
struct PendingFrameInfo {
    is_sync: bool,
    frame_nr: FrameNumber,
    source: VideoSource,
    presentation_timestamp: Time,
    decode_timestamp: Time,
    duration: Option<Time>,
}

struct GpuSyncDecoder {
    decoder: re_gpu_video::H264Decoder,
    input_format: InputFormat,

    /// Reused conversion buffer for AVCC input.
    annexb_buffer: Vec<u8>,

    /// Chunk metadata keyed by presentation timestamp, waiting for the decoded frame.
    ///
    /// Frames come out in presentation order, so everything at an earlier
    /// timestamp than an emitted frame is stale and gets pruned.
    pending_frame_infos: BTreeMap<i64, PendingFrameInfo>,

    /// Shared with [`GpuDecoder::min_num_samples_to_enqueue_ahead`].
    reorder_delay: Arc<AtomicUsize>,
}

impl GpuSyncDecoder {
    fn emit_frames(
        &mut self,
        frames: Vec<re_gpu_video::DecodedFrame>,
        output_sender: &Sender<FrameResult>,
    ) {
        for frame in frames {
            let Some(info) = self.pending_frame_infos.remove(&frame.pts) else {
                // Can't happen: every frame's timestamp comes from a submitted chunk.
                re_log::warn_once!(
                    "GPU-decoded video frame at timestamp {} has no matching chunk metadata",
                    frame.pts
                );
                continue;
            };
            self.pending_frame_infos = self.pending_frame_infos.split_off(&frame.pts);

            let _send_error = output_sender.send(Ok(Frame {
                content: FrameContent::GpuTexture(Box::new(frame)),
                info: FrameInfo {
                    is_sync: Some(info.is_sync),
                    frame_nr: Some(info.frame_nr),
                    source: Some(info.source),
                    presentation_timestamp: info.presentation_timestamp,
                    duration: info.duration,
                    latest_decode_timestamp: Some(info.decode_timestamp),
                },
            }));
        }
    }
}

impl SyncDecoder for GpuSyncDecoder {
    fn submit_chunk(
        &mut self,
        should_stop: &AtomicBool,
        chunk: Chunk,
        output_sender: &Sender<FrameResult>,
    ) {
        re_tracing::profile_function!();

        if should_stop.load(Ordering::Relaxed) {
            return;
        }

        let data: &[u8] = match &mut self.input_format {
            InputFormat::AnnexB => &chunk.data,
            InputFormat::Avcc { avcc, state } => {
                self.annexb_buffer.clear();
                if let Err(err) =
                    write_avc_chunk_to_nalu_stream(avcc, &mut self.annexb_buffer, &chunk, state)
                {
                    let _send_error =
                        output_sender.send(Err(DecodeError::BadAvccData(err.to_string())));
                    return;
                }
                &self.annexb_buffer
            }
        };

        let pts = chunk.presentation_timestamp.0;
        self.pending_frame_infos.insert(
            pts,
            PendingFrameInfo {
                is_sync: chunk.is_sync,
                frame_nr: chunk.frame_nr,
                source: chunk.source,
                presentation_timestamp: chunk.presentation_timestamp,
                decode_timestamp: chunk.decode_timestamp,
                duration: chunk.duration,
            },
        );

        match self.decoder.push_access_unit(data, pts) {
            Ok(frames) => self.emit_frames(frames, output_sender),
            Err(err) => {
                let _send_error = output_sender.send(Err(DecodeError::GpuVideo(Arc::new(err))));
            }
        }

        self.reorder_delay
            .store(self.decoder.reorder_delay(), Ordering::Relaxed);
    }

    fn end_of_video(&mut self, output_sender: &Sender<FrameResult>) {
        match self.decoder.flush() {
            Ok(frames) => self.emit_frames(frames, output_sender),
            Err(err) => {
                let _send_error = output_sender.send(Err(DecodeError::GpuVideo(Arc::new(err))));
            }
        }
    }

    fn reset(&mut self, video_descr: &VideoDataDescription) {
        self.decoder.reset();
        self.input_format = InputFormat::from_descr(video_descr);
        self.pending_frame_infos.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_capabilities() -> H264DecodeCapabilities {
        H264DecodeCapabilities {
            min_coded_extent: [16, 16],
            max_coded_extent: [4096, 4096],
            max_dpb_slots: 17,
            max_active_references: 16,
            max_level_idc: 51,
        }
    }

    fn test_encoding_details() -> VideoEncodingDetails {
        VideoEncodingDetails {
            codec_string: "avc1.64002A".to_owned(),
            coded_dimensions: [1920, 1080],
            bit_depth: Some(8),
            chroma_subsampling: Some(ChromaSubsamplingModes::Yuv420),
            frames_only: Some(true),
            stsd: None,
        }
    }

    #[test]
    fn high_profile_4_2_0_stream_is_supported() {
        let details = test_encoding_details();
        assert_eq!(
            h264_unsupported_reason(Some(&details), &test_capabilities()),
            None
        );
    }

    #[test]
    fn stream_without_encoding_details_is_not_ruled_out() {
        assert_eq!(h264_unsupported_reason(None, &test_capabilities()), None);
    }

    #[test]
    fn chroma_subsampling_other_than_4_2_0_is_unsupported() {
        let details = VideoEncodingDetails {
            chroma_subsampling: Some(ChromaSubsamplingModes::Yuv422),
            ..test_encoding_details()
        };
        assert!(
            h264_unsupported_reason(Some(&details), &test_capabilities())
                .is_some_and(|reason| reason.contains("4:2:2"))
        );
    }

    #[test]
    fn interlaced_video_is_unsupported() {
        let details = VideoEncodingDetails {
            frames_only: Some(false),
            ..test_encoding_details()
        };
        assert!(
            h264_unsupported_reason(Some(&details), &test_capabilities())
                .is_some_and(|reason| reason.contains("interlaced"))
        );
    }

    #[test]
    fn level_above_the_device_maximum_is_unsupported() {
        // `avc1.640034` is High profile at level 5.2, one step above the device's 5.1.
        let details = VideoEncodingDetails {
            codec_string: "avc1.640034".to_owned(),
            ..test_encoding_details()
        };
        assert!(
            h264_unsupported_reason(Some(&details), &test_capabilities())
                .is_some_and(|reason| reason.contains("level 52"))
        );
    }

    #[test]
    fn codec_string_of_unexpected_shape_leaves_profile_and_level_unknown() {
        assert_eq!(h264_profile_and_level("avc1"), None);
        assert_eq!(h264_profile_and_level("avc1.64"), None);
        assert_eq!(h264_profile_and_level("hvc1.1.6.L93.B0"), None);
        assert_eq!(h264_profile_and_level("avc1.64002A"), Some((100, 42)));
    }
}
