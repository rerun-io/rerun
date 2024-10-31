//! Send video data to `ffmpeg` over CLI to decode it.

use std::{collections::BTreeMap, io::Write, sync::Arc};

use crossbeam::channel::{Receiver, Sender};
use ffmpeg_sidecar::{
    child::FfmpegChild,
    command::FfmpegCommand,
    event::{FfmpegEvent, LogLevel},
};

use crate::Time;

use super::{AsyncDecoder, Chunk, Frame, OutputCallback};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to start ffmppeg: {0}")]
    FailedToStartFfmpeg(std::io::Error),

    #[error("Failed to get stdin handle")]
    NoStdin,

    #[error("Failed to get iterator: {0}")]
    NoIterator(String),

    #[error("No frame info received, this is a likely a bug in Rerun")]
    NoFrameInfo,

    #[error("Failed to write data to ffmpeg: {0}")]
    FailedToWriteToFfmpeg(std::io::Error),

    #[error("Bad video data: {0}")]
    BadVideoData(String),

    #[error("FFMPEG error: {0}")]
    Ffmpeg(String),

    #[error("FFMPEG fatal error: {0}")]
    FfmpegFatal(String),

    #[error("FFMPEG IPC error: {0}")]
    FfmpegSidecar(String),

    #[error("FFMPEG exited unexpectedly with code {0:?}")]
    FfmpegUnexpectedExit(Option<std::process::ExitStatus>),

    #[error("Failed to send video frame info to the ffmpeg read thread.")]
    BrokenFrameInfoChannel,
}

impl From<Error> for super::Error {
    fn from(err: Error) -> Self {
        Self::Ffmpeg(std::sync::Arc::new(err))
    }
}

/// ffmpeg does not tell us the timestamp/duration of a given frame, so we need to remember it.
#[derive(Clone)]
struct FfmpegFrameInfo {
    presentation_timestamp: Time,
    duration: Time,
    decode_timestamp: Time,
}

enum FfmpegFrameData {
    Chunk(Chunk),
    EndOfStream,
}

struct FfmpegProcessAndListener {
    ffmpeg: FfmpegChild,

    /// For sending frame timestamps to the ffmpeg listener thread.
    frame_info_tx: Sender<FfmpegFrameInfo>,
    /// For sending chunks to the ffmpeg write thread.
    frame_data_tx: Sender<FfmpegFrameData>,

    listen_thread: Option<std::thread::JoinHandle<()>>,
    write_thread: Option<std::thread::JoinHandle<()>>,
}

impl FfmpegProcessAndListener {
    fn new(
        debug_name: &str,
        on_output: Arc<OutputCallback>,
        avcc: re_mp4::Avc1Box,
    ) -> Result<Self, Error> {
        re_tracing::profile_function!();

        let mut ffmpeg = FfmpegCommand::new()
            .hide_banner()
            // "Reduce the latency introduced by buffering during initial input streams analysis."
            //.arg("-fflags nobuffer")
            //
            // .. instead use these more aggressive options found here
            // https://stackoverflow.com/a/49273163
            .args([
                "-probesize",
                "32", // 32 bytes is the minimum probe size.
                "-analyzeduration",
                "0",
            ])
            // Keep in mind that all arguments that are about the input, need to go before!
            .format("h264") // TODO(andreas): should we check ahead of time whether this is available?
            //.fps_mode("0")
            .input("-") // stdin is our input!
            // h264 bitstreams doesn't have timestamp information. Whatever ffmpeg tries to make up about timing & framerates is wrong!
            // If we don't tell it to just pass the frames through, variable framerate (VFR) video will just not play at all.
            .fps_mode("passthrough")
            // TODO(andreas): at least do `rgba`. But we could also do `yuv420p` for instance if that's what the video is specifying
            // (should be faster overall at no quality loss if the video is in this format).
            // Check `ffmpeg -pix_fmts` for full list.
            .rawvideo() // Output rgb24 on stdout.
            .spawn()
            .map_err(Error::FailedToStartFfmpeg)?;

        let ffmpeg_iterator = ffmpeg
            .iter()
            .map_err(|err| Error::NoIterator(err.to_string()))?;

        let (frame_info_tx, frame_info_rx) = crossbeam::channel::unbounded();
        let (frame_data_tx, frame_data_rx) = crossbeam::channel::unbounded();

        let listen_thread = std::thread::Builder::new()
            .name(format!("ffmpeg-reader for {debug_name}"))
            .spawn({
                let on_output = on_output.clone();
                let debug_name = debug_name.to_owned();
                move || {
                    read_ffmpeg_output(
                        &debug_name,
                        ffmpeg_iterator,
                        &frame_info_rx,
                        on_output.as_ref(),
                    );
                }
            })
            .expect("Failed to spawn ffmpeg listener thread");

        let write_thread = std::thread::Builder::new()
            .name(format!("ffmpeg-writer for {debug_name}"))
            .spawn({
                let ffmpeg_stdin = ffmpeg.take_stdin().ok_or(Error::NoStdin)?;
                move || {
                    write_ffmpeg_input(ffmpeg_stdin, &frame_data_rx, on_output.as_ref(), &avcc);
                }
            })
            .expect("Failed to spawn ffmpeg writer thread");

        Ok(Self {
            ffmpeg,
            frame_info_tx,
            frame_data_tx,
            listen_thread: Some(listen_thread),
            write_thread: Some(write_thread),
        })
    }
}

impl Drop for FfmpegProcessAndListener {
    fn drop(&mut self) {
        re_tracing::profile_function!();
        self.frame_data_tx.send(FfmpegFrameData::EndOfStream).ok();
        self.ffmpeg.kill().ok();

        if let Some(write_thread) = self.write_thread.take() {
            if write_thread.join().is_err() {
                re_log::error!("Failed to join ffmpeg listener thread.");
            }
        }
        if let Some(listen_thread) = self.listen_thread.take() {
            if listen_thread.join().is_err() {
                re_log::error!("Failed to join ffmpeg listener thread.");
            }
        }
    }
}

fn write_ffmpeg_input(
    mut ffmpeg_stdin: std::process::ChildStdin,
    frame_data_rx: &Receiver<FfmpegFrameData>,
    on_output: &OutputCallback,
    avcc: &re_mp4::Avc1Box,
) {
    let mut state = NaluStreamState::default();

    while let Ok(data) = frame_data_rx.recv() {
        let chunk = match data {
            FfmpegFrameData::Chunk(chunk) => chunk,
            FfmpegFrameData::EndOfStream => break,
        };

        if let Err(err) =
            write_avc_chunk_to_nalu_stream(avcc, &mut ffmpeg_stdin, &chunk, &mut state)
        {
            // This is unlikely to improve! Ffmpeg process likely died.
            // By exiting here we hang up on the channel, making future attempts to push into it fail which should cause a reset eventually.
            let should_exit = matches!(err, Error::FailedToWriteToFfmpeg(_));
            (on_output)(Err(err.into()));
            if should_exit {
                return;
            }
        } else {
            ffmpeg_stdin.flush().ok();
        }
    }
}

fn read_ffmpeg_output(
    debug_name: &str,
    ffmpeg_iterator: ffmpeg_sidecar::iter::FfmpegIterator,
    frame_info_rx: &Receiver<FfmpegFrameInfo>,
    on_output: &OutputCallback,
) {
    /// Ignore some common output from ffmpeg:
    fn should_ignore_log_msg(msg: &str) -> bool {
        let patterns = [
            "Duration: N/A, bitrate: N/A",
            "frame=    0 fps=0.0 q=0.0 size=       0kB time=N/A bitrate=N/A speed=N/A",
            "Metadata:",
            // TODO(andreas): we should just handle yuv420p directly!
            "No accelerated colorspace conversion found from yuv420p to rgb24",
            "Stream mapping:",
            // We actually don't even want it to estimate a framerate!
            "not enough frames to estimate rate",
        ];

        for pattern in patterns {
            if msg.contains(pattern) {
                return true;
            }
        }

        false
    }

    // Pending frames, sorted by their presentation timestamp.
    let mut pending_frame_infos = BTreeMap::new();
    let mut highest_dts = Time(i64::MIN); // Highest dts encountered so far.

    for event in ffmpeg_iterator {
        #[allow(clippy::match_same_arms)]
        match event {
            FfmpegEvent::Log(LogLevel::Info, msg) => {
                if !should_ignore_log_msg(&msg) {
                    re_log::trace!("{debug_name} decoder: {msg}");
                }
            }

            FfmpegEvent::Log(LogLevel::Warning, msg) => {
                if !should_ignore_log_msg(&msg) {
                    re_log::warn_once!("{debug_name} decoder: {msg}");
                }
            }

            FfmpegEvent::Log(LogLevel::Error, msg) => {
                on_output(Err(Error::Ffmpeg(msg).into()));
            }

            FfmpegEvent::Log(LogLevel::Fatal, msg) => {
                on_output(Err(Error::FfmpegFatal(msg).into()));
                return;
            }

            FfmpegEvent::Log(LogLevel::Unknown, msg) => {
                if msg.contains("system signals, hard exiting") {
                    // That was probably us, killing the process.
                    re_log::debug!("ffmpeg process for {debug_name} was killed");
                    return;
                }
                if !should_ignore_log_msg(&msg) {
                    re_log::debug!("{debug_name} decoder: {msg}");
                }
            }

            FfmpegEvent::LogEOF => {
                // This event proceeds `FfmpegEvent::Done`.
                // This happens on `pkill ffmpeg`, for instance.
            }

            FfmpegEvent::Error(error) => {
                on_output(Err(Error::FfmpegSidecar(error).into()));
            }

            // Usefuless info in these:
            FfmpegEvent::ParsedInput(input) => {
                re_log::trace!("{debug_name} decoder: {input:?}");
            }
            FfmpegEvent::ParsedOutput(output) => {
                re_log::trace!("{debug_name} decoder: {output:?}");
            }

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

                re_log::trace!(
                    "{debug_name} decoder input: {stream_type} {format} {pix_fmt} {width}x{height} @ {fps} FPS"
                );

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
                re_log::trace!(
                    "{debug_name} decoder output: {stream_type} {format} {pix_fmt} {width}x{height} @ {fps} FPS"
                );

                debug_assert_eq!(stream_type.to_ascii_lowercase(), "video");
            }

            FfmpegEvent::Progress(_) => {
                // We can get out frame number etc here to know how far behind we are.
                // By default this triggers every 5s.
            }

            FfmpegEvent::OutputFrame(frame) => {
                // DTS <= PTS
                // chunk sorted by DTS

                // Frames come in in PTS order, but "frame info" comes in in DTS order!
                //
                // Whenever the highest known DTS is behind the PTS, we need to wait until the DTS catches up.
                // Otherwise, we'd assign the wrong PTS to the frame that just came in.
                //
                // Example how how presentation timestamps and decode timestamps
                // can play out in the presence of B-frames to illustrate this:
                //    PTS: 1 4 2 3
                //    DTS: 1 2 3 4
                // Stream: I P B B
                let frame_info = loop {
                    if pending_frame_infos
                        .first_key_value()
                        .map_or(true, |(pts, _)| *pts > highest_dts)
                    {
                        let Ok(frame_info) = frame_info_rx.try_recv() else {
                            re_log::debug!(
                                "{debug_name} ffmpeg decoder frame info channel disconnected"
                            );
                            return;
                        };

                        // If the decodetimestamp did not increase, we're probably seeking backwards!
                        // We'd expect the video player to do a reset prior to that and close the channel as part of that, but we may not have noticed that in here yet!
                        // In any case, we'll have to just run with this as the new highest timestamp, not much else we can do.
                        if highest_dts < frame_info.decode_timestamp {
                            re_log::warn_once!("Video decode timestamps are expected to monotonically increase unless there was a decoder reset. This is probably a bug in Rerun.");
                        }
                        highest_dts = frame_info.decode_timestamp;

                        pending_frame_infos.insert(frame_info.presentation_timestamp, frame_info);
                    } else {
                        // There must be an element here, otherwise we wouldn't be in this branch.
                        #[allow(clippy::unwrap_used)]
                        break pending_frame_infos.pop_first().unwrap().1;
                    }
                };

                let ffmpeg_sidecar::event::OutputVideoFrame {
                    frame_num: _, // This is made up by ffmpeg sidecar.
                    pix_fmt,
                    width,
                    height,
                    data,
                    output_index: _, // This is the stream index. for all we do it's always 0.
                    timestamp: _, // This is a timestamp made up by ffmpeg_sidecar based on limited information it has.
                } = frame;

                re_log::trace!(
                    "{debug_name} received frame: dts {:?} cts {:?} fmt {pix_fmt:?} size {width}x{height}",
                    frame_info.decode_timestamp,
                    frame_info.presentation_timestamp
                );

                debug_assert_eq!(pix_fmt, "rgb24");
                debug_assert_eq!(width as usize * height as usize * 3, data.len());

                on_output(Ok(super::Frame {
                    content: super::FrameContent {
                        data,
                        width,
                        height,
                        format: crate::PixelFormat::Rgb8Unorm,
                    },
                    info: super::FrameInfo {
                        presentation_timestamp: frame_info.presentation_timestamp,
                        duration: frame_info.duration,
                        latest_decode_timestamp: Some(frame_info.decode_timestamp),
                    },
                }));
            }

            FfmpegEvent::Done => {
                // This happens on `pkill ffmpeg`, for instance.
                re_log::debug!("{debug_name}'s ffmpeg is Done");
                return;
            }

            // TODO: handle all events
            event => re_log::debug!("{debug_name} event: {event:?}"),
        }
    }
}

/// Decode H.264 video via ffmpeg over CLI
pub struct FfmpegCliH264Decoder {
    debug_name: String,
    ffmpeg: FfmpegProcessAndListener,
    avcc: re_mp4::Avc1Box,
    on_output: Arc<OutputCallback>,
}

impl FfmpegCliH264Decoder {
    pub fn new(
        debug_name: String,
        avcc: re_mp4::Avc1Box,
        on_output: impl Fn(super::Result<Frame>) + Send + Sync + 'static,
    ) -> Result<Self, Error> {
        re_tracing::profile_function!();

        let on_output = Arc::new(on_output);
        let ffmpeg = FfmpegProcessAndListener::new(&debug_name, on_output.clone(), avcc.clone())?;

        Ok(Self {
            debug_name,
            ffmpeg,
            avcc,
            on_output,
        })
    }
}

impl AsyncDecoder for FfmpegCliH264Decoder {
    fn submit_chunk(&mut self, chunk: super::Chunk) -> super::Result<()> {
        re_tracing::profile_function!();

        // We send the information about this chunk first.
        // This assumes each sample/chunk will result in exactly one frame.
        // If this assumption is not held, we will get weird errors, like videos playing to slowly.
        let frame_info = FfmpegFrameInfo {
            presentation_timestamp: chunk.presentation_timestamp,
            decode_timestamp: chunk.decode_timestamp,
            duration: chunk.duration,
        };
        let chunk = FfmpegFrameData::Chunk(chunk);

        if self.ffmpeg.frame_info_tx.send(frame_info).is_err()
            || self.ffmpeg.frame_data_tx.send(chunk).is_err()
        {
            let err = super::Error::Ffmpeg(Arc::new(
                if let Ok(exit_code) = self.ffmpeg.ffmpeg.as_inner_mut().try_wait() {
                    Error::FfmpegUnexpectedExit(exit_code)
                } else {
                    Error::BrokenFrameInfoChannel
                },
            ));

            // Report the error on the decoding stream.
            (self.on_output)(Err(err.clone()));
            return Err(err);
        }

        Ok(())
    }

    fn reset(&mut self) -> super::Result<()> {
        re_log::debug!("Resetting ffmpeg decoder {}", self.debug_name);
        self.ffmpeg = FfmpegProcessAndListener::new(
            &self.debug_name,
            self.on_output.clone(),
            self.avcc.clone(),
        )?;
        Ok(())
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
) -> Result<(), Error> {
    re_tracing::profile_function!();
    let avcc = &avcc.avcc;

    // Append SPS (Sequence Parameter Set) & PPS (Picture Parameter Set) NAL unit whenever encountering
    // an IDR frame unless the previous frame was an IDR frame.
    // TODO(andreas): Should we detect this rather from the NALU stream rather than the samples?
    if chunk.is_sync && !state.previous_frame_was_idr {
        for sps in &avcc.sequence_parameter_sets {
            nalu_stream
                .write_all(NAL_START_CODE)
                .map_err(Error::FailedToWriteToFfmpeg)?;
            nalu_stream
                .write_all(&sps.bytes)
                .map_err(Error::FailedToWriteToFfmpeg)?;
        }
        for pps in &avcc.picture_parameter_sets {
            nalu_stream
                .write_all(NAL_START_CODE)
                .map_err(Error::FailedToWriteToFfmpeg)?;
            nalu_stream
                .write_all(&pps.bytes)
                .map_err(Error::FailedToWriteToFfmpeg)?;
        }
        state.previous_frame_was_idr = true;
    } else {
        state.previous_frame_was_idr = false;
    }

    // A single chunk may consist of multiple NAL units, each of which need our special treatment.
    // (most of the time it's 1:1, but there might be extra NAL units for info, especially at the start).
    let mut buffer_offset: usize = 0;
    let sample_end = chunk.data.len();
    while buffer_offset < sample_end {
        re_tracing::profile_scope!("write_nalu");

        // Each NAL unit in mp4 is prefixed with a length prefix.
        // In Annex B this doesn't exist.
        let length_prefix_size = avcc.length_size_minus_one as usize + 1;

        if sample_end < buffer_offset + length_prefix_size {
            return Err(Error::BadVideoData(
                "Not enough bytes to fit the length prefix".to_owned(),
            ));
        }

        let nal_unit_size = match length_prefix_size {
            1 => chunk.data[buffer_offset] as usize,

            2 => u16::from_be_bytes(
                #[allow(clippy::unwrap_used)] // can't fail
                chunk.data[buffer_offset..(buffer_offset + 2)]
                    .try_into()
                    .unwrap(),
            ) as usize,

            4 => u32::from_be_bytes(
                #[allow(clippy::unwrap_used)] // can't fail
                chunk.data[buffer_offset..(buffer_offset + 4)]
                    .try_into()
                    .unwrap(),
            ) as usize,

            _ => {
                return Err(Error::BadVideoData(format!(
                    "Bad length prefix size: {length_prefix_size}"
                )));
            }
        };

        let data_start = buffer_offset + length_prefix_size; // Skip the size.
        let data_end = buffer_offset + nal_unit_size + length_prefix_size;

        if chunk.data.len() < data_end {
            return Err(Error::BadVideoData("Not enough bytes to".to_owned()));
        }

        let nal_header = NalHeader(chunk.data[data_start]);
        re_log::trace!(
            "nal_header: {:?}, {}",
            nal_header.unit_type(),
            nal_header.ref_idc()
        );

        let data = &chunk.data[data_start..data_end];

        nalu_stream
            .write_all(NAL_START_CODE)
            .map_err(Error::FailedToWriteToFfmpeg)?;

        // Note that we don't have to insert "emulation prevention bytes" since mp4 NALU still use them.
        // (unlike the NAL start code, the presentation bytes are part of the NAL spec!)

        re_tracing::profile_scope!("write_bytes", data.len().to_string());
        nalu_stream
            .write_all(data)
            .map_err(Error::FailedToWriteToFfmpeg)?;

        buffer_offset = data_end;
    }

    // Write an Access Unit Delimiter (AUD) NAL unit to the stream to signal the end of an access unit.
    // This can help with ffmpeg picking up NALs right away before seeing the next chunk.
    nalu_stream
        .write_all(NAL_START_CODE)
        .map_err(Error::FailedToWriteToFfmpeg)?;
    nalu_stream
        .write_all(&[
            NalHeader::new(NalUnitType::AccessUnitDelimiter, 3).0,
            // Two arbitrary bytes? 0000 worked as well, but this is what
            // https://stackoverflow.com/a/44394025/ uses. Couldn't figure out the rules for this.
            0xFF,
            0x80,
        ])
        .map_err(Error::FailedToWriteToFfmpeg)?;

    Ok(())
}

/// Possible values for `nal_unit_type` field in `nal_unit`.
///
/// Encodes to 5 bits.
/// Via:
/// * <https://docs.rs/less-avc/0.1.5/src/less_avc/nal_unit.rs.html#232/>
/// * <https://github.com/FFmpeg/FFmpeg/blob/87068b9600daa522e3f45b5501ecd487a3c0be57/libavcodec/h264.h#L33>
#[derive(PartialEq, Eq)]
#[non_exhaustive]
#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum NalUnitType {
    /// Unspecified
    Unspecified = 0,

    /// Coded slice of a non-IDR picture
    CodedSliceOfANonIDRPicture = 1,

    /// Coded slice data partition A
    CodedSliceDataPartitionA = 2,

    /// Coded slice data partition B
    CodedSliceDataPartitionB = 3,

    /// Coded slice data partition C
    CodedSliceDataPartitionC = 4,

    /// Coded slice of an IDR picture
    CodedSliceOfAnIDRPicture = 5,

    /// Supplemental enhancement information (SEI)
    SupplementalEnhancementInformation = 6,

    /// Sequence parameter set
    SequenceParameterSet = 7,

    /// Picture parameter set
    PictureParameterSet = 8,

    /// Signals the end of a NAL unit.
    AccessUnitDelimiter = 9,

    EndSequence = 10,
    EndStream = 11,
    FillerData = 12,
    SequenceParameterSetExt = 13,

    /// Header type not listed here.
    Other,
}

/// Header of the "Network Abstraction Layer" unit that is used by H.264/AVC & H.265/HEVC.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct NalHeader(pub u8);

impl NalHeader {
    pub const fn new(unit_type: NalUnitType, ref_idc: u8) -> Self {
        Self((unit_type as u8) | (ref_idc << 5))
    }

    pub fn unit_type(self) -> NalUnitType {
        match self.0 & 0b11111 {
            0 => NalUnitType::Unspecified,
            1 => NalUnitType::CodedSliceOfANonIDRPicture,
            2 => NalUnitType::CodedSliceDataPartitionA,
            3 => NalUnitType::CodedSliceDataPartitionB,
            4 => NalUnitType::CodedSliceDataPartitionC,
            5 => NalUnitType::CodedSliceOfAnIDRPicture,
            6 => NalUnitType::SupplementalEnhancementInformation,
            7 => NalUnitType::SequenceParameterSet,
            8 => NalUnitType::PictureParameterSet,
            9 => NalUnitType::AccessUnitDelimiter,
            10 => NalUnitType::EndSequence,
            11 => NalUnitType::EndStream,
            12 => NalUnitType::FillerData,
            13 => NalUnitType::SequenceParameterSetExt,
            _ => NalUnitType::Other,
        }
    }

    /// Ref idc is a value from 0-3 that tells us how "important" the frame/sample is.
    ///
    /// For details see:
    /// <https://yumichan.net/video-processing/video-compression/breif-description-of-nal_ref_idc-value-in-h-246-nalu/>
    pub fn ref_idc(self) -> u8 {
        (self.0 >> 5) & 0b11
    }
}
