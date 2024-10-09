//! Send video data to `ffmpeg` over CLI to decode it.

use crossbeam::channel::{Receiver, Sender, TryRecvError};
use ffmpeg_sidecar::{
    command::FfmpegCommand,
    event::{FfmpegEvent, LogLevel},
};

use crate::Time;

use super::{async_decoder_wrapper::SyncDecoder, Frame, Result};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to start ffmppeg: {0}")]
    FailedToStartFfmpeg(std::io::Error),

    #[error("Failed to get stdin handle")]
    NoStdin,

    #[error("Failed to get iterator: {0}")]
    NoIterator(String),

    #[error("There's a bug in Rerun")]
    NoFrameInfo,
}

impl From<Error> for super::Error {
    fn from(err: Error) -> Self {
        Self::Ffmpeg(std::sync::Arc::new(err))
    }
}

/// ffmpeg does not tell us the timestamp/duration of a given frame, so we need to remember it.
struct FrameInfo {
    /// Monotonic index, from start
    frame_num: u32,

    timestamp: Time,
    duration: Time,
}

/// Decode H.264 video via ffmpeg over CLI

pub struct FfmpegCliH264Decoder {
    /// Monotonically increasing
    frame_num: u32,

    /// How we send more data to the ffmpeg process
    ffmpeg_stdin: std::process::ChildStdin,

    /// For sending frame timestamps to the decoder thread
    frame_info_tx: Sender<FrameInfo>,

    /// How we receive new frames back from ffmpeg
    frame_rx: Receiver<super::Result<Frame>>,

    avcc: re_mp4::Avc1Box,
}

impl FfmpegCliH264Decoder {
    pub fn new(avcc: re_mp4::Avc1Box) -> Result<Self> {
        re_tracing::profile_function!();

        let mut ffmpeg = {
            re_tracing::profile_scope!("spawn-ffmpeg");

            FfmpegCommand::new()
                .hide_banner()
                // Keep in mind that all arguments that are about the input, need to go before!
                .format("h264") // High risk here: What's is available?
                .input("-") // stdin is our input!
                .rawvideo() // Output rgb24 on stdout. (TODO(emilk) for later: any format we can read directly on re_renderer would be better!)
                .spawn()
                .map_err(Error::FailedToStartFfmpeg)?
        };

        let ffmpeg_stdin = ffmpeg.take_stdin().ok_or(Error::NoStdin)?;
        let ffmpeg_iterator = ffmpeg
            .iter()
            .map_err(|err| Error::NoIterator(err.to_string()))?;

        let (frame_info_tx, frame_info_rx) = crossbeam::channel::unbounded();
        let (frame_tx, frame_rx) = crossbeam::channel::unbounded();

        std::thread::Builder::new()
            .name("ffmpeg-reader".to_owned())
            .spawn(move || {
                read_ffmpeg_output(ffmpeg_iterator, &frame_info_rx, &frame_tx);
                re_log::debug!("Shutting down ffmpeg");
            })
            .expect("Failed to spawn ffmpeg thread");

        Ok(Self {
            frame_num: 0,
            ffmpeg_stdin,
            frame_info_tx,
            frame_rx,
            avcc,
        })
    }
}

fn read_ffmpeg_output(
    ffmpeg_iterator: ffmpeg_sidecar::iter::FfmpegIterator,
    frame_info_rx: &Receiver<FrameInfo>,
    frame_tx: &Sender<super::Result<Frame>>,
) {
    for event in ffmpeg_iterator {
        #[allow(clippy::match_same_arms)]
        match event {
            FfmpegEvent::Log(LogLevel::Info, msg) => {
                re_log::debug!("{msg}");
            }

            FfmpegEvent::Log(LogLevel::Warning, msg) => {
                if !msg.contains("No accelerated colorspace conversion found from yuv420p to rgb24")
                {
                    re_log::warn_once!("{msg}");
                }
            }

            FfmpegEvent::Log(LogLevel::Error, msg) => {
                // TODO: report errors
                re_log::error_once!("{msg}");
            }

            // Usefuless info in these:
            FfmpegEvent::ParsedInput(_) => {}
            FfmpegEvent::ParsedOutput(_) => {}
            FfmpegEvent::ParsedStreamMapping(_) => {}

            FfmpegEvent::ParsedInputStream(stream) => {
                let ffmpeg_sidecar::event::AVStream {
                    stream_type,
                    format,
                    pix_fmt, // Often 'yuv420p'
                    width,
                    height,
                    fps,
                    ..
                } = stream;

                re_log::debug!("ParsedInputStream {stream_type} {format} {pix_fmt} {width}x{height} @ {fps} FPS");

                debug_assert_eq!(stream_type.to_ascii_lowercase(), "video");
            }

            FfmpegEvent::ParsedOutputStream(stream) => {
                // This just repeats what we told ffmpeg to output, e.g. "rawvideo rgb24"
                let ffmpeg_sidecar::event::AVStream {
                    stream_type,
                    format,
                    pix_fmt,
                    width,
                    height,
                    fps,
                    ..
                } = stream;

                re_log::debug!("ParsedOutputStream {stream_type} {format} {pix_fmt} {width}x{height} @ {fps} FPS");

                debug_assert_eq!(stream_type.to_ascii_lowercase(), "video");
            }

            FfmpegEvent::Progress(_) => {
                // We can get out frame number etc here to know how far behind we are.
            }

            FfmpegEvent::OutputFrame(frame) => {
                // NOTE: `frame.timestamp` is monotonically increasing,
                // and is not the actual timestamp in the stream.

                let frame_info: FrameInfo = match frame_info_rx.try_recv() {
                    Ok(frame_info) => frame_info,

                    Err(TryRecvError::Disconnected) => {
                        re_log::debug!("Receiver disconnected");
                        return;
                    }

                    Err(TryRecvError::Empty) => {
                        // This shouldn't happen
                        if frame_tx.send(Err(Error::NoFrameInfo.into())).is_err() {
                            re_log::warn!("Got no frame-info, and failed to send error");
                        }
                        return;
                    }
                };

                let ffmpeg_sidecar::event::OutputVideoFrame {
                    frame_num,
                    pix_fmt,
                    width,
                    height,
                    data,
                    ..
                } = frame;

                debug_assert_eq!(
                    frame_info.frame_num, frame_num,
                    "We are out-of-sync with ffmpeg"
                ); // TODO: fix somehow

                re_log::trace!("Received frame {frame_num}: fmt {pix_fmt:?} size {width}x{height}");

                debug_assert_eq!(pix_fmt, "rgb24");
                debug_assert_eq!(width as usize * height as usize * 3, data.len());

                if frame_tx
                    .send(Ok(super::Frame {
                        width,
                        height,
                        data,
                        format: crate::PixelFormat::Rgb8Unorm,
                        presentation_timestamp: frame_info.timestamp,
                        duration: frame_info.duration,
                    }))
                    .is_err()
                {
                    re_log::debug!("Receiver disconnected");
                    return;
                }
            }

            FfmpegEvent::Done => {
                re_log::debug!("ffmpeg is Done");
                return;
            }
            // TODO: handle all events
            event => re_log::debug!("Event: {event:?}"),
        }
    }
}

impl SyncDecoder for FfmpegCliH264Decoder {
    fn submit_chunk(
        &mut self,
        should_stop: &std::sync::atomic::AtomicBool,
        chunk: super::Chunk,
        on_output: &super::OutputCallback,
    ) {
        re_tracing::profile_function!();

        // NOTE: this assumes each sample/chunk will result in exactly one frame.
        self.frame_info_tx.send(FrameInfo {
            frame_num: self.frame_num,
            timestamp: chunk.composition_timestamp,
            duration: chunk.duration,
        });

        // NOTE: a 60 FPS video can go for two years before wrapping a u32.
        self.frame_num = self.frame_num.wrapping_add(1);

        let mut state = NaluStreamState::default();
        write_avc_chunk_to_nalu_stream(&self.avcc, &mut self.ffmpeg_stdin, &chunk, &mut state)
            .unwrap();
        // consider writing samples while at the same time reading frames, for even lower latency
        // and maybe reuse the same ffmpeg process.

        // TODO: handle errors
        while let Ok(frame_result) = self.frame_rx.try_recv() {
            if should_stop.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            on_output(frame_result);
        }
    }

    fn reset(&mut self) {
        // TODO: restart ffmpeg process
    }
}

/// Before every NAL unit, here is a nal start code.
/// Can also be 2 bytes of 0x00 and 1 byte of 0x01.
///
/// This is used in byte stream formats such as h264 files.
/// Packet transform systems (RTP) may omit these.
pub const NAL_START_CODE: &[u8] = &[0x00, 0x00, 0x00, 0x01];

#[derive(Default)]
struct NaluStreamState {
    previous_frame_was_idr: bool,
}

fn write_avc_chunk_to_nalu_stream(
    avcc: &re_mp4::Avc1Box,
    nalu_stream: &mut dyn std::io::Write,
    chunk: &super::Chunk,
    state: &mut NaluStreamState,
) -> Result<(), Box<dyn std::error::Error>> {
    re_tracing::profile_function!();
    let avcc = &avcc.avcc;

    // Append SPS (Sequence Parameter Set) & PPS (Picture Parameter Set) NAL unit whenever encountering
    // an IDR frame unless the previous frame was an IDR frame.
    // TODO(andreas): Should we detect this rather from the NALU stream rather than the samples?
    if chunk.is_sync && !state.previous_frame_was_idr {
        for sps in &avcc.sequence_parameter_sets {
            nalu_stream.write_all(NAL_START_CODE)?;
            nalu_stream.write_all(&sps.bytes)?;
        }
        for pps in &avcc.picture_parameter_sets {
            nalu_stream.write_all(NAL_START_CODE)?;
            nalu_stream.write_all(&pps.bytes)?;
        }
        state.previous_frame_was_idr = true;
    } else {
        state.previous_frame_was_idr = false;
    }

    // A single cjhunk may consist of multiple NAL units, each of which need our special treatment.
    // (most of the time it's 1:1, but there might be extra NAL units for info, especially at the start).
    let mut buffer_offset: usize = 0;
    let sample_end = chunk.data.len();
    while buffer_offset < sample_end {
        re_tracing::profile_scope!("nalu");

        // Each NAL unit in mp4 is prefixed with a length prefix.
        // In Annex B this doesn't exist.
        let length_prefix_size = avcc.length_size_minus_one as usize + 1;

        // TODO: improve the error handling here.
        let nal_unit_size = match length_prefix_size {
            4 => u32::from_be_bytes(
                chunk.data[buffer_offset..(buffer_offset + 4)]
                    .try_into()
                    .unwrap(),
            ) as usize,
            2 => u16::from_be_bytes(
                chunk.data[buffer_offset..(buffer_offset + 2)]
                    .try_into()
                    .unwrap(),
            ) as usize,
            1 => chunk.data[buffer_offset] as usize,
            _ => panic!("invalid length prefix size"),
        };
        //re_log::debug!("nal unit size: {}", nal_unit_size);

        if chunk.data.len() < nal_unit_size {
            panic!(
                "sample size {} is smaller than nal unit size {nal_unit_size}",
                chunk.data.len()
            );
        }

        nalu_stream.write_all(NAL_START_CODE)?;
        let data_start = buffer_offset + length_prefix_size; // Skip the size.
        let data_end = buffer_offset + nal_unit_size + length_prefix_size;
        let data = &chunk.data[data_start..data_end];

        // Note that we don't have to insert "emulation prevention bytes" since mp4 NALU still use them.
        // (unlike the NAL start code, the presentation bytes are part of the NAL spec!)

        nalu_stream.write_all(data)?;

        buffer_offset = data_end;
    }

    Ok(())
}
