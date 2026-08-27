//! H.264 decoding on the GPU via `re_gpu_video`, producing frames that never leave VRAM.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use re_gpu_video::GpuVideoContext;

use super::sync_decoder_wrapper::{SyncDecoder, SyncDecoderWrapper};
use super::{AsyncDecoder, Chunk, DecodeError, Frame, FrameContent, FrameInfo, FrameResult};
use crate::h264::write_avc_chunk_to_nalu_stream;
use crate::nalu::AnnexBStreamState;
use crate::{FrameNumber, Sender, Time, VideoDataDescription, VideoSource};

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

    /// H.264 in MP4: prepend SPS/PPS to each IDR, otherwise length-prefix → annex-b.
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
        let frames = self.decoder.flush();
        self.emit_frames(frames, output_sender);
    }

    fn reset(&mut self, video_descr: &VideoDataDescription) {
        self.decoder.reset();
        self.input_format = InputFormat::from_descr(video_descr);
        self.pending_frame_infos.clear();
    }
}
