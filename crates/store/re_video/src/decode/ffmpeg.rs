//! Send video data to `ffmpeg` over CLI to decode it.

use crossbeam::channel::Receiver;
use ffmpeg_sidecar::{
    child::FfmpegChild,
    command::FfmpegCommand,
    event::{FfmpegEvent, LogLevel},
};

use crate::{Time, Timescale};

use super::{Frame, Result, SyncDecoder};

/// Decode H.264 video via ffmpeg over CLI

pub struct FfmpegCliH264Decoder {
    /// How we send more data to the ffmpeg process
    ffmpeg_stdin: std::process::ChildStdin,

    /// How we receive new frames back from ffmpeg
    frame_rx: Receiver<Result<Frame>>,

    avcc: re_mp4::Avc1Box,
    timescale: Timescale,
}

impl FfmpegCliH264Decoder {
    pub fn new(avcc: re_mp4::Avc1Box, timescale: Timescale) -> Result<Self> {
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
                .expect("Failed to spawn ffmpeg")
        };

        let mut ffmpeg_stdin = ffmpeg.take_stdin().unwrap();
        let ffmpeg_iterator = ffmpeg.iter().unwrap();

        let (frame_tx, frame_rx) = crossbeam::channel::unbounded();

        let thread_handle = std::thread::Builder::new()
            .name("ffmpeg-reader".to_owned())
            .spawn(move || {
                for event in ffmpeg_iterator {
                    match event {
                        FfmpegEvent::Log(LogLevel::Warning, msg) => re_log::warn_once!("{msg}"),
                        FfmpegEvent::Log(LogLevel::Error, msg) => re_log::error_once!("{msg}"), // TODO: report errors
                        FfmpegEvent::Progress(p) => {
                            re_log::debug!("Progress: {}", p.time)
                        }
                        FfmpegEvent::OutputFrame(frame) => {
                            re_log::trace!(
                                "Received frame: d[0] {} time {:?} fmt {:?} size {}x{}",
                                frame.data[0],
                                frame.timestamp,
                                frame.pix_fmt,
                                frame.width,
                                frame.height
                            );

                            debug_assert_eq!(frame.pix_fmt, "rgb24");
                            debug_assert_eq!(
                                frame.width as usize * frame.height as usize * 3,
                                frame.data.len()
                            );

                            frame_tx.send(Ok(super::Frame {
                                width: frame.width,
                                height: frame.height,
                                data: frame.data,
                                format: crate::PixelFormat::Rgb8Unorm,
                                timestamp: Time::from_secs(frame.timestamp as f64, timescale),
                                duration: Time::from_secs(0.1, timescale), // TODO
                            })); // TODO: handle disconnect
                        }
                        // TODO: handle all events
                        event => re_log::debug!("Event: {event:?}"),
                    }
                }
                re_log::debug!("Shutting down ffmpeg");
            });

        Ok(Self {
            ffmpeg_stdin,
            frame_rx,
            avcc,
            timescale,
        })
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

        let mut state = NaluStreamState::default();
        write_avc_chunk_to_nalu_stream(&self.avcc, &mut self.ffmpeg_stdin, &chunk, &mut state)
            .unwrap();
        // consider writing samples while at the same time reading frames, for even lower latency
        // and maybe reuse the same ffmpeg process.

        // TODO: handle errors
        while let Ok(frame_result) = self.frame_rx.try_recv() {
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
        for sps in (&avcc.sequence_parameter_sets).iter() {
            nalu_stream.write_all(&NAL_START_CODE)?;
            nalu_stream.write_all(&sps.bytes)?;
        }
        for pps in (&avcc.picture_parameter_sets).iter() {
            nalu_stream.write_all(&NAL_START_CODE)?;
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

        nalu_stream.write_all(&NAL_START_CODE)?;
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
