//! Send video data to `ffmpeg` over CLI to decode it.

use std::collections::BTreeMap;
use std::process::ChildStdin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crossbeam::channel::{Receiver, SendError, Sender};
use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use h264_reader::nal::UnitType;
use parking_lot::Mutex;

use super::version::FFmpegVersionParseError;
use crate::decode::ffmpeg_cli::{
    FFMPEG_MINIMUM_VERSION_MAJOR, FFMPEG_MINIMUM_VERSION_MINOR, FFmpegVersion,
};
use crate::decode::{
    AsyncDecoder, Chunk, DecodeError, Frame, FrameContent, FrameInfo, FrameResult,
};
use crate::demux::ChromaSubsamplingModes;
use crate::h264::write_avc_chunk_to_nalu_stream;
use crate::h265::write_hevc_chunk_to_nalu_stream;
use crate::nalu::{ANNEXB_NAL_START_CODE, AnnexBStreamState, AnnexBStreamWriteError};
use crate::{PixelFormat, Time, VideoDataDescription, VideoEncodingDetails};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Couldn't find an installation of the FFmpeg executable.")]
    FFmpegNotInstalled,

    #[error("Failed to start FFmpeg: {0}")]
    FailedToStartFfmpeg(std::io::Error),

    #[error(
        "FFmpeg version is {actual_version}. Only versions >= {minimum_version_major}.{minimum_version_minor} are officially supported."
    )]
    UnsupportedFFmpegVersion {
        actual_version: FFmpegVersion,
        minimum_version_major: u32,
        minimum_version_minor: u32,
    },

    // TODO(andreas): This error can have a variety of reasons and is as such redundant to some of the others.
    // It works with an inner error because some of the error sources are behind an anyhow::Error inside of ffmpeg-sidecar.
    #[error(transparent)]
    FailedToDetermineFFmpegVersion(FFmpegVersionParseError),

    #[error("Failed to get stdin handle")]
    NoStdin,

    #[error("Failed to get iterator: {0}")]
    NoIterator(String),

    #[error("No frame info received, this is a likely a bug in Rerun")]
    NoFrameInfo,

    #[error("Failed to write data to FFmpeg: {0}")]
    FailedToWriteToFfmpeg(std::io::Error),

    #[error("Bad video data: {0}")]
    BadVideoData(String),

    #[error("FFmpeg error: {0}")]
    Ffmpeg(String),

    #[error("FFmpeg fatal error: {0}")]
    FfmpegFatal(String),

    #[error("FFmpeg IPC error: {0}")]
    FfmpegSidecar(String),

    #[error("FFmpeg exited unexpectedly with code {0:?}")]
    FfmpegUnexpectedExit(Option<std::process::ExitStatus>),

    #[error("FFmpeg output a non-image chunk when we expected only images.")]
    UnexpectedFfmpegOutputChunk,

    #[error("Failed to send video frame info to the FFmpeg read thread.")]
    BrokenFrameInfoChannel,

    #[error("Failed to parse sequence parameter set.")]
    SpsParsing,
}

impl Error {
    pub fn should_request_more_frames(&self) -> bool {
        // Restarting ffmpeg can recover from some decoder internal errors.
        matches!(
            self,
            Self::Ffmpeg(_) | Self::FfmpegFatal(_) | Self::UnexpectedFfmpegOutputChunk
        )
    }
}

impl From<Error> for DecodeError {
    fn from(err: Error) -> Self {
        Self::Ffmpeg(std::sync::Arc::new(err))
    }
}

impl From<AnnexBStreamWriteError> for Error {
    fn from(err: AnnexBStreamWriteError) -> Self {
        match err {
            AnnexBStreamWriteError::BadVideoData(msg) => Self::BadVideoData(msg),
            AnnexBStreamWriteError::FailedToWriteToStream(err) => Self::FailedToWriteToFfmpeg(err),
        }
    }
}

/// ffmpeg does not tell us the timestamp/duration of a given frame, so we need to remember it.
#[derive(Clone, Debug)]
struct FFmpegFrameInfo {
    /// The start of a new group of pictures?
    ///
    /// This probably means this is a _keyframe_, and that and entire frame
    /// can be decoded from only this one sample (though I'm not 100% sure).
    is_sync: bool,

    /// Which sample in the video is this from?
    ///
    /// In MP4, one sample is one frame, but we may be reordering samples when decoding.
    ///
    /// This is the order of which the samples appear in the container,
    /// which is usually ordered by [`Self::decode_timestamp`].
    sample_idx: usize,

    /// Which frame is this?
    ///
    /// This is on the assumption that each sample produces a single frame,
    /// which is true for MP4.
    ///
    /// This is the index of frames ordered by [`Self::presentation_timestamp`].
    frame_nr: u32,

    presentation_timestamp: Time,
    duration: Option<Time>,
    decode_timestamp: Time,
}

enum FFmpegFrameData {
    Chunk(Chunk),
    Quit,
}

/// Wraps an stdin with a shared shutdown boolean.
struct StdinWithShutdown {
    shutdown: Arc<AtomicBool>,
    stdin: ChildStdin,
}

impl StdinWithShutdown {
    // Don't use `std::io::ErrorKind::Interrupted` because it has special meaning for default implementations of the `Write` trait,
    // causing it to continue.
    const SHUTDOWN_ERROR_KIND: std::io::ErrorKind = std::io::ErrorKind::Other;
}

impl std::io::Write for StdinWithShutdown {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.shutdown.load(Ordering::Acquire) {
            Err(std::io::Error::new(Self::SHUTDOWN_ERROR_KIND, "shutdown"))
        } else {
            self.stdin.write(buf)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.shutdown.load(Ordering::Acquire) {
            Err(std::io::Error::new(Self::SHUTDOWN_ERROR_KIND, "shutdown"))
        } else {
            self.stdin.flush()
        }
    }
}

/// Output sender is mutex protected `Option` so that we can stop sending output frames on ffmpeg shutdown by setting it to `None`.
type OutputSender = Mutex<Option<Sender<FrameResult>>>;

/// Send a result to the output sender.
fn send_output(
    output_sender: &OutputSender,
    result: FrameResult,
) -> Result<(), SendError<FrameResult>> {
    let output_sender_guard = output_sender.lock();
    if let Some(output_sender) = output_sender_guard.as_ref() {
        output_sender.send(result)
    } else {
        Err(SendError(result))
    }
}

/// Encapsulates the running of an ffmpeg process.
///
/// Dropping this closes the process.
struct FFmpegProcessAndListener {
    ffmpeg: FfmpegChild,

    /// For sending frame timestamps to the ffmpeg listener thread.
    frame_info_tx: Sender<FFmpegFrameInfo>,

    /// For sending chunks to the ffmpeg write thread.
    frame_data_tx: Sender<FFmpegFrameData>,

    listen_thread: Option<std::thread::JoinHandle<()>>,
    write_thread: Option<std::thread::JoinHandle<()>>,

    /// Number of samples submitted to ffmpeg that has not yet been outputted by ffmpeg.
    ///
    /// This counter is for debugging purposes only.
    num_outstanding_frames: Arc<AtomicI32>,

    /// If true, the write thread will not report errors. Used upon exit, so the write thread won't log spam on the hung up stdin.
    stdin_shutdown: Arc<AtomicBool>,

    /// Any output results will be sent to this channel sender.
    output_sender: Arc<OutputSender>,
}

impl FFmpegProcessAndListener {
    fn new(
        debug_name: &str,
        output_sender: Sender<FrameResult>,
        encoding_details: &Option<VideoEncodingDetails>,
        ffmpeg_path: Option<&std::path::Path>,
        codec: &crate::VideoCodec,
    ) -> Result<Self, Error> {
        re_tracing::profile_function!();

        // TODO(andreas): should get SPS also without AVCC from ongoing stream.

        let (pixel_format, ffmpeg_pix_fmt) = if let Some(chroma_subsampling) =
            encoding_details.as_ref().and_then(|e| e.chroma_subsampling)
        {
            // We always get planar layouts back from ffmpeg.
            let (layout, ffmpeg_pix_fmt) = match chroma_subsampling {
                ChromaSubsamplingModes::Yuv444 => {
                    (crate::decode::YuvPixelLayout::Y_U_V444, "yuvj444p")
                }
                ChromaSubsamplingModes::Yuv422 => {
                    (crate::decode::YuvPixelLayout::Y_U_V422, "yuvj422p")
                }
                ChromaSubsamplingModes::Yuv420 => {
                    (crate::decode::YuvPixelLayout::Y_U_V420, "yuvj420p")
                }
                ChromaSubsamplingModes::Monochrome => (crate::decode::YuvPixelLayout::Y400, "gray"),
            };

            let pixel_format = PixelFormat::Yuv {
                layout,
                // Unfortunately the color range is an entirely different thing to parse as it's part of optional Video Usability Information (VUI).
                //
                // We instead just always tell ffmpeg to give us full range, see`-color_range` below.
                // Note that yuvj4xy family of formats fulfill the same function. They according to this post
                // https://www.facebook.com/permalink.php?story_fbid=2413101932257643&id=100006735798590
                // they are still not quite passed through everywhere. So we'll just use both.
                range: crate::decode::YuvRange::Full,
                // Again, instead of parsing this out we tell ffmpeg to give us BT.709.
                coefficients: crate::decode::YuvMatrixCoefficients::Bt709,
            };

            (pixel_format, ffmpeg_pix_fmt)
        } else {
            (PixelFormat::Rgba8Unorm, "rgba")
        };

        let mut ffmpeg_command = if let Some(ffmpeg_path) = ffmpeg_path {
            FfmpegCommand::new_with_path(ffmpeg_path)
        } else {
            FfmpegCommand::new()
        };

        let codec_str = match codec {
            crate::VideoCodec::H264 => "h264",
            crate::VideoCodec::H265 => "hevc",
            _ => unreachable!(),
        };

        let mut ffmpeg = ffmpeg_command
            // Keep banner enabled so we can check on the version more easily.
            //.hide_banner()
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
            .format(codec_str) // TODO(andreas): should we check ahead of time whether this is available?
            //.fps_mode("0")
            .input("-") // stdin is our input!
            // h264 bitstreams doesn't have timestamp information. Whatever ffmpeg tries to make up about timing & framerates is wrong!
            // If we don't tell it to just pass the frames through, variable framerate (VFR) video will just not play at all.
            .fps_mode("passthrough")
            .pix_fmt(ffmpeg_pix_fmt)
            // ffmpeg-sidecar's .rawvideo() sets pix_fmt to rgb24, we don't want that.
            .args(["-f", "rawvideo"])
            // This should be taken care of by the yuvj formats, but let's be explicit again that we want full color range
            .args(["-color_range", "2"]) // 2 == pc/full
            // Besides the less and less common Bt601, this is the only space we support right now, so let ffmpeg do the conversion.
            // TODO(andreas): It seems that FFmpeg 7.0 handles this as I expect, but FFmpeg 7.1 consistently gives me the wrong colors on the Bunny test clip.
            // (tested Windows with both FFmpeg 7.0 and 7.1, tested Mac with 7.1. More rigorous testing and comparing is required!)
            .args(["-colorspace", "1"]) // 1 == Bt.709
            .output("-") // Output to stdout.
            .spawn()
            .map_err(Error::FailedToStartFfmpeg)?;

        let ffmpeg_iterator = ffmpeg
            .iter()
            .map_err(|err| Error::NoIterator(err.to_string()))?;

        let (frame_info_tx, frame_info_rx) = crossbeam::channel::unbounded();
        let (frame_data_tx, frame_data_rx) = crossbeam::channel::unbounded();

        let num_outstanding_frames = Arc::new(AtomicI32::new(0));
        let stdin_shutdown = Arc::new(AtomicBool::new(false));

        // Mutex protect `output_sender` so that we can shut down the threads at a defined point in time at which we
        // no longer receive any new frames or errors from this process.
        let output_sender = Arc::new(Mutex::new(Some(output_sender)));

        // Reads the output from the ffmpeg process:
        let listen_thread = std::thread::Builder::new()
            .name(format!("ffmpeg-reader for {debug_name}"))
            .spawn({
                let output_sender = output_sender.clone();
                let debug_name = debug_name.to_owned();
                let ffmpeg_path = ffmpeg_path.map(|p| p.to_owned());
                let outstanding_frames = num_outstanding_frames.clone();
                move || {
                    read_ffmpeg_output(
                        &debug_name,
                        ffmpeg_path.as_deref(),
                        ffmpeg_iterator,
                        &frame_info_rx,
                        &pixel_format,
                        &outstanding_frames,
                        &output_sender,
                    );
                }
            })
            .expect("Failed to spawn ffmpeg listener thread");

        let codec_meta = encoding_details
            .as_ref()
            .and_then(|e| e.stsd.as_ref())
            .and_then(CodecMeta::from_stsd)
            .unwrap_or(CodecMeta::RawBytestream);

        // Writes video data to the ffmpeg process:
        let write_thread = std::thread::Builder::new()
            .name(format!("ffmpeg-writer for {debug_name}"))
            .spawn({
                let output_sender = output_sender.clone();
                let ffmpeg_stdin = ffmpeg.take_stdin().ok_or(Error::NoStdin)?;
                let mut ffmpeg_stdin = StdinWithShutdown {
                    stdin: ffmpeg_stdin,
                    shutdown: stdin_shutdown.clone(),
                };
                move || {
                    write_ffmpeg_input(
                        &mut ffmpeg_stdin,
                        &frame_data_rx,
                        &output_sender,
                        &codec_meta,
                    );
                }
            })
            .expect("Failed to spawn ffmpeg writer thread");

        Ok(Self {
            ffmpeg,
            num_outstanding_frames,
            frame_info_tx,
            frame_data_tx,
            listen_thread: Some(listen_thread),
            write_thread: Some(write_thread),
            stdin_shutdown,
            output_sender,
        })
    }

    fn submit_chunk(&mut self, chunk: Chunk) -> Result<(), Error> {
        // We send the information about this chunk first.
        // Chunks are defined to always yield a single frame.
        let frame_info = FFmpegFrameInfo {
            is_sync: chunk.is_sync,
            sample_idx: chunk.sample_idx,
            frame_nr: chunk.frame_nr,
            presentation_timestamp: chunk.presentation_timestamp,
            decode_timestamp: chunk.decode_timestamp,
            duration: chunk.duration,
        };

        let data = FFmpegFrameData::Chunk(chunk);

        if self.frame_info_tx.send(frame_info).is_err() || self.frame_data_tx.send(data).is_err() {
            Err(
                if let Ok(exit_code) = self.ffmpeg.as_inner_mut().try_wait() {
                    Error::FfmpegUnexpectedExit(exit_code)
                } else {
                    Error::BrokenFrameInfoChannel
                },
            )
        } else {
            self.num_outstanding_frames.fetch_add(1, Ordering::Relaxed);

            Ok(())
        }
    }

    fn end_of_video(&self) {
        // Close stdin. That will let ffmpeg know that it should flush its buffers.
        self.frame_data_tx.send(FFmpegFrameData::Quit).ok();
    }
}

impl Drop for FFmpegProcessAndListener {
    fn drop(&mut self) {
        re_tracing::profile_function!();

        // Stop all outputs from being written to - any attempt from here on out will fail and cause thread shutdown.
        // This way, we ensure all ongoing writes are finished and won't get any more on_output callbacks from this process
        // before we take any other action on the shutdown sequence.
        {
            self.output_sender.lock().take();
        }

        // Notify (potentially wake up) the stdin write thread to stop it (it might be sleeping).
        self.frame_data_tx.send(FFmpegFrameData::Quit).ok();
        // Kill stdin for the write thread. This helps cancelling ongoing stream write operations.
        self.stdin_shutdown.store(true, Ordering::Release);

        // Kill the ffmpeg process itself.
        // It's important that we wait for it to finish, otherwise the process may enter a zombie state, see https://en.wikipedia.org/wiki/Zombie_process.
        // Also, a nice side effect of wait is that it ensures that stdin is closed.
        // This should wake up the listen thread if it is sleeping, but that may take a while.
        {
            let kill_result = self.ffmpeg.kill();
            let wait_result = self.ffmpeg.wait();
            if kill_result.is_err() || wait_result.is_err() {
                re_log::debug!(
                    "FFmpeg kill result: {:?}, wait result: {:?}",
                    kill_result,
                    wait_result
                );
            }
        }

        // Unfortunately, even with the above measures, it can still happen that the listen threads take occasionally 100ms and more to shut down.
        // (very much depending on the system & OS, typical times may be low with large outliers)
        // It is crucial that the threads come down eventually and rather timely so to avoid leaking resources.
        // However, in order to avoid stalls, we'll let them finish in parallel.
        //
        // Since we disconnected the `on_output` callback from them, they won't influence any new instances.
        if false {
            {
                re_tracing::profile_scope!("shutdown write thread");
                if let Some(write_thread) = self.write_thread.take()
                    && write_thread.join().is_err()
                {
                    re_log::error!("Failed to join ffmpeg listener thread.");
                }
            }
            {
                re_tracing::profile_scope!("shutdown listen thread");
                if let Some(listen_thread) = self.listen_thread.take()
                    && listen_thread.join().is_err()
                {
                    re_log::error!("Failed to join ffmpeg listener thread.");
                }
            }
        }

        re_log::trace!(
            "Outstanding frames after shutting down ffmpeg: {}",
            self.num_outstanding_frames.load(Ordering::Relaxed)
        );
    }
}

fn write_ffmpeg_input(
    ffmpeg_stdin: &mut dyn std::io::Write,
    frame_data_rx: &Receiver<FFmpegFrameData>,
    output_sender: &OutputSender,
    codec_meta: &CodecMeta,
) {
    let mut state = AnnexBStreamState::default();

    while let Ok(data) = frame_data_rx.recv() {
        let chunk = match data {
            FFmpegFrameData::Chunk(chunk) => chunk,
            FFmpegFrameData::Quit => {
                // Try to flush out the last frames from ffmpeg with an EndSequence/EndStream NAL units.
                // Unfortunatelt this doesn't help, at least not for https://github.com/rerun-io/rerun/issues/8073
                let end_nals: Vec<u8> = [
                    ANNEXB_NAL_START_CODE,
                    &[UnitType::EndOfSeq.id()],
                    ANNEXB_NAL_START_CODE,
                    &[UnitType::EndOfStream.id()],
                ]
                .concat();
                write_bytes(ffmpeg_stdin, &end_nals).ok();

                // NOTE(emilk): I've also tried writing `NalUnitType::AccessUnitDelimiter` here, but to no avail.

                ffmpeg_stdin.flush().ok();

                break;
            }
        };

        let write_result = match codec_meta {
            CodecMeta::Avc(avcc) => {
                write_avc_chunk_to_nalu_stream(avcc, ffmpeg_stdin, &chunk, &mut state)
                    .map_err(Error::from)
            }
            CodecMeta::Hevc(hvcc) => {
                write_hevc_chunk_to_nalu_stream(hvcc, ffmpeg_stdin, &chunk, &mut state)
                    .map_err(Error::from)
            }
            CodecMeta::RawBytestream => write_bytes(ffmpeg_stdin, &chunk.data),
        };

        if let Err(err) = write_result {
            let write_error = matches!(err, Error::FailedToWriteToFfmpeg(_));
            if send_output(output_sender, Err(err.into())).is_err() {
                // Other side hung up on us, we're done.
                // This can happen if for some reason the video decoding was aborted, don't treat it as an error.
                return;
            }

            if write_error {
                // This is unlikely to improve! Ffmpeg process likely died.
                // By exiting here we hang up on the channel, making future attempts to push into it fail which should cause a reset eventually.
                return;
            }
        } else {
            ffmpeg_stdin.flush().ok();
            re_log::trace!("Wrote chunk {} to ffmpeg", chunk.sample_idx);
        }
    }
}

struct FrameBuffer {
    /// Received frame-infos, waiting to be matched to output frames.
    ///
    /// Key is the frame number, making this list sorted in presentation order.
    pending: BTreeMap<u32, FFmpegFrameInfo>,

    /// The frame number of the next frame if we had any so far.
    ///
    /// `None` if we haven't received any frames yet since the last decoder reset.
    next_frame_nr: Option<u32>,
}

impl FrameBuffer {
    fn new() -> Self {
        Self {
            pending: BTreeMap::new(),
            next_frame_nr: None,
        }
    }

    fn on_frame(
        &mut self,
        pixel_format: &PixelFormat,
        frame_info_rx: &Receiver<FFmpegFrameInfo>,
        frame: ffmpeg_sidecar::event::OutputVideoFrame,
    ) -> Option<Frame> {
        // ffmpeg gives us raw images, but we have to pair them up with frame infos.
        //
        // We input frames into ffmpeg in decode (DTS) order, and so that's
        // also the order we will receive the `FrameInfo`s from `frame_info_rx`.
        //
        // However, `ffmpeg` will re-order the frames to output them in presentation (PTS) order.
        // We want to accurately match the `FrameInfo` with its corresponding output frame.
        // To do that, we need to buffer frames that come out of ffmpeg.
        let frame_info = loop {
            let oldest_pending = self.pending.first_entry();

            if let Some(oldest_pending) = oldest_pending {
                let frame_info = oldest_pending.get();

                let is_next_expected_frame = if let Some(next_frame_nr) = self.next_frame_nr {
                    frame_info.frame_nr == next_frame_nr
                } else {
                    // This is the first frame we're receiving since the last decoder reset.
                    // We expect to always start at a sync-frame.
                    // Note that sync frames do _not_ imply DTS == PTS since DTS may start with a negative offset for some videos.
                    debug_assert!(
                        frame_info.is_sync,
                        "Expected first received frame after a decoder reset to be a sync-frame (start of group of pictures)."
                    );
                    true
                };

                if is_next_expected_frame {
                    self.next_frame_nr = Some(frame_info.frame_nr + 1);
                    break oldest_pending.remove_entry().1;
                }
            }

            // We haven't received the frame info for this frame yet.
            let Ok(frame_info) = frame_info_rx.recv() else {
                re_log::trace!("frame-tx channel closed, stopping ffmpeg decoder");
                return None;
            };
            self.pending.insert(frame_info.frame_nr, frame_info);
        };

        let ffmpeg_sidecar::event::OutputVideoFrame {
            frame_num: _, // This is made up by ffmpeg sidecar.
            pix_fmt: _,   // TODO(emilk); use this instead of the `pixel_format` argument.
            width,
            height,
            data,
            output_index: _, // This is the stream index. for all we do it's always 0.
            timestamp: _, // This is a timestamp made up by ffmpeg_sidecar based on limited information it has.
        } = frame;

        debug_assert_eq!(
            data.len() * 8,
            (width * height * pixel_format.bits_per_pixel()) as usize
        );

        Some(Frame {
            content: FrameContent {
                data,
                width,
                height,
                format: pixel_format.clone(),
            },
            info: FrameInfo {
                is_sync: Some(frame_info.is_sync),
                sample_idx: Some(frame_info.sample_idx),
                frame_nr: Some(frame_info.frame_nr),
                presentation_timestamp: frame_info.presentation_timestamp,
                latest_decode_timestamp: Some(frame_info.decode_timestamp),
                duration: frame_info.duration,
            },
        })
    }
}

fn read_ffmpeg_output(
    debug_name: &str,
    ffmpeg_path: Option<&std::path::Path>,
    ffmpeg_iterator: ffmpeg_sidecar::iter::FfmpegIterator,
    frame_info_rx: &Receiver<FFmpegFrameInfo>,
    pixel_format: &PixelFormat,
    outstanding_frames: &AtomicI32,
    output_sender: &OutputSender,
) -> Option<()> {
    // Before we do anything else - make sure the ffmpeg version is compatible:
    // Ok to block here - we're in a background thread.
    let ffmpeg_version_result = FFmpegVersion::for_executable_blocking(ffmpeg_path);
    if let Err(err) = check_ffmpeg_version(ffmpeg_version_result) {
        send_output(output_sender, Err(err.into())).ok();
        return None;
    }

    let mut buffer = FrameBuffer::new();

    for event in ffmpeg_iterator {
        #[expect(clippy::match_same_arms)]
        match event {
            FfmpegEvent::Log(level, msg) => {
                if msg.contains("system signals, hard exiting") {
                    // That was probably us, killing the process.
                    re_log::debug!("FFmpeg process for {debug_name} was killed");
                    return None;
                }

                let ignore = match level {
                    LogLevel::Info | LogLevel::Unknown | LogLevel::Warning => {
                        should_ignore_log_msg(&msg)
                    }
                    LogLevel::Error | LogLevel::Fatal => false,
                };

                if !ignore {
                    let msg = sanitize_ffmpeg_log_message(&msg);
                    match level {
                        LogLevel::Info => {
                            re_log::trace!("{debug_name} decoder: {msg}");
                        }
                        LogLevel::Warning | LogLevel::Unknown => {
                            // Older ffmpeg versions don't flag their warnings as such and end up as `LogLevel::Unknown`.
                            re_log::warn_once!("{debug_name} decoder: {msg}");
                        }
                        LogLevel::Error => {
                            send_output(output_sender, Err(Error::Ffmpeg(msg).into())).ok()?;
                        }
                        LogLevel::Fatal => {
                            send_output(output_sender, Err(Error::FfmpegFatal(msg).into())).ok()?;
                        }
                    }
                }
            }

            FfmpegEvent::LogEOF => {
                // This event proceeds `FfmpegEvent::Done`.
                // This happens on `pkill ffmpeg`, for instance.
            }

            FfmpegEvent::Error(error) => {
                // An error in ffmpeg sidecar itself, rather than ffmpeg.
                send_output(output_sender, Err(Error::FfmpegSidecar(error).into())).ok()?;
            }

            FfmpegEvent::ParsedInput(input) => {
                re_log::trace!("{debug_name} decoder: {input:?}");
            }

            FfmpegEvent::ParsedOutput(output) => {
                re_log::trace!("{debug_name} decoder: {output:?}");
            }

            FfmpegEvent::ParsedStreamMapping(_) => {
                // This reports what input streams ffmpeg maps to which output streams.
                // Very unspectecular in our case as know that we map h264 video to raw video.
            }

            FfmpegEvent::ParsedInputStream(stream) => {
                let ffmpeg_sidecar::event::Stream {
                    format,
                    language,
                    parent_index,
                    stream_index,
                    raw_log_message: _,
                    type_specific_data,
                } = &stream;

                re_log::trace!(
                    "{debug_name} decoder input: {format} ({language}) parent: {parent_index}, index: {stream_index}, stream data: {type_specific_data:?}"
                );

                debug_assert!(stream.is_video());
            }

            FfmpegEvent::ParsedOutputStream(stream) => {
                // This just repeats what we told ffmpeg to output, e.g. "rawvideo rgb24"
                let ffmpeg_sidecar::event::Stream {
                    format,
                    language,
                    parent_index,
                    stream_index,
                    raw_log_message: _,
                    type_specific_data,
                } = &stream;

                re_log::trace!(
                    "{debug_name} decoder output: {format} ({language}) parent: {parent_index}, index: {stream_index}, stream data: {type_specific_data:?}"
                );

                debug_assert!(stream.is_video());
            }

            FfmpegEvent::Progress(_) => {
                // We can get out frame number etc here to know how far behind we are.
                // By default this triggers every 5s.
            }

            FfmpegEvent::OutputFrame(ffmpeg_frame) => {
                outstanding_frames.fetch_sub(1, Ordering::Relaxed);

                let frame_num = ffmpeg_frame.frame_num; // sequence-number made up by ffmpeg sidecar.

                let frame = buffer.on_frame(pixel_format, frame_info_rx, ffmpeg_frame)?;

                {
                    // Log
                    let FrameContent {
                        width,
                        height,
                        format,
                        ..
                    } = &frame.content;
                    re_log::trace!(
                        "{debug_name} received frame {frame_num}: sample {sample_idx:?} dts {dts:?} pts {pts:?} fmt {format:?} size {width}x{height}. buffered: {num_buffered}, outstanding: {num_outstanding}",
                        sample_idx = frame.info.sample_idx,
                        dts = frame.info.latest_decode_timestamp,
                        pts = frame.info.presentation_timestamp,
                        num_buffered = buffer.pending.len(),
                        num_outstanding = outstanding_frames.load(Ordering::Relaxed),
                    );
                }

                send_output(output_sender, Ok(frame)).ok()?;
            }

            FfmpegEvent::Done => {
                // This happens on `pkill ffmpeg`, for instance.
                re_log::trace!("{debug_name}'s ffmpeg is Done");
                return None;
            }

            FfmpegEvent::ParsedVersion(ffmpeg_version) => {
                fn download_advice() -> String {
                    if let Ok(download_url) = ffmpeg_sidecar::download::ffmpeg_download_url() {
                        format!(
                            "\nYou can download an up to date version for your system at {download_url}."
                        )
                    } else {
                        String::new()
                    }
                }

                if let Some(ffmpeg_version) = FFmpegVersion::parse(&ffmpeg_version.version) {
                    re_log::debug_once!("Parsed FFmpeg version: {ffmpeg_version}");

                    if !ffmpeg_version.is_compatible() {
                        send_output(
                            output_sender,
                            Err(Error::UnsupportedFFmpegVersion {
                                actual_version: ffmpeg_version,
                                minimum_version_major: FFMPEG_MINIMUM_VERSION_MAJOR,
                                minimum_version_minor: FFMPEG_MINIMUM_VERSION_MINOR,
                            }
                            .into()),
                        )
                        .ok()?;
                    }
                } else {
                    re_log::warn_once!(
                        "Failed to parse FFmpeg version: {}{}",
                        ffmpeg_version.version,
                        download_advice()
                    );
                }
            }

            FfmpegEvent::ParsedConfiguration(ffmpeg_configuration) => {
                re_log::debug_once!(
                    "FFmpeg configuration: {:?}",
                    ffmpeg_configuration.configuration
                );
            }

            FfmpegEvent::ParsedDuration(_) => {
                // ffmpeg has no way of knowing the duration of the stream. Whatever it might make up is wrong.
            }

            FfmpegEvent::OutputChunk(_) => {
                // Something went seriously wrong if we end up here.
                re_log::error!("Unexpected ffmpeg output chunk for {debug_name}");
                send_output(
                    output_sender,
                    Err(Error::UnexpectedFfmpegOutputChunk.into()),
                )
                .ok()?;
                return None;
            }
        }
    }

    Some(())
}

/// Decode video via ffmpeg over CLI
pub struct FFmpegCliDecoder {
    debug_name: String,
    // Restarted on reset
    ffmpeg: FFmpegProcessAndListener,
    output_sender: Sender<FrameResult>,
    ffmpeg_path: Option<std::path::PathBuf>,
    codec: crate::VideoCodec,
}

impl FFmpegCliDecoder {
    pub fn new(
        debug_name: String,
        encoding_details: &Option<VideoEncodingDetails>,
        output_sender: Sender<FrameResult>,
        ffmpeg_path: Option<std::path::PathBuf>,
        codec: &crate::VideoCodec,
    ) -> Result<Self, Error> {
        re_tracing::profile_function!();

        // Check the version once ahead of running FFmpeg, if we can get it without blocking.
        // We also check it in a background thread, but getting the error
        // early is preferable:
        if let std::task::Poll::Ready(ffmpeg_version_result) =
            FFmpegVersion::for_executable_poll(ffmpeg_path.as_deref())
        {
            check_ffmpeg_version(ffmpeg_version_result)?;
        }

        let ffmpeg = FFmpegProcessAndListener::new(
            &debug_name,
            output_sender.clone(),
            encoding_details,
            ffmpeg_path.as_deref(),
            codec,
        )?;

        Ok(Self {
            debug_name,
            ffmpeg,
            output_sender,
            ffmpeg_path,
            codec: *codec,
        })
    }
}

fn check_ffmpeg_version(
    ffmpeg_version_result: Result<FFmpegVersion, FFmpegVersionParseError>,
) -> Result<(), Error> {
    match ffmpeg_version_result {
        Ok(version) => {
            if version.is_compatible() {
                Ok(())
            } else {
                Err(Error::UnsupportedFFmpegVersion {
                    actual_version: version,
                    minimum_version_major: FFMPEG_MINIMUM_VERSION_MAJOR,
                    minimum_version_minor: FFMPEG_MINIMUM_VERSION_MINOR,
                })
            }
        }

        Err(FFmpegVersionParseError::FFmpegNotFound(_)) => Err(Error::FFmpegNotInstalled),

        Err(FFmpegVersionParseError::ParseVersion { raw_version }) => {
            // This happens quite often, don't fail playing video over it!
            re_log::warn_once!("Failed to parse FFmpeg version: {raw_version}");
            Ok(())
        }

        Err(err) => Err(Error::FailedToDetermineFFmpegVersion(err)),
    }
}

impl AsyncDecoder for FFmpegCliDecoder {
    fn submit_chunk(&mut self, chunk: Chunk) -> crate::decode::Result<()> {
        re_tracing::profile_function!();

        if let Err(err) = self.ffmpeg.submit_chunk(chunk) {
            let err = DecodeError::from(err);

            // Report the error on the decoding stream aswell.
            self.output_sender.send(Err(err.clone())).ok();

            Err(err)
        } else {
            Ok(())
        }
    }

    fn end_of_video(&mut self) -> crate::decode::Result<()> {
        re_log::trace!("End of video - flushing ffmpeg decoder {}", self.debug_name);
        self.ffmpeg.end_of_video();
        Ok(())
    }

    fn reset(&mut self, video_descr: &VideoDataDescription) -> crate::decode::Result<()> {
        re_tracing::profile_function!();
        re_log::trace!("Resetting ffmpeg decoder {}", self.debug_name);
        self.ffmpeg = FFmpegProcessAndListener::new(
            &self.debug_name,
            self.output_sender.clone(),
            &video_descr.encoding_details,
            self.ffmpeg_path.as_deref(),
            &self.codec,
        )?;
        Ok(())
    }

    fn min_num_samples_to_enqueue_ahead(&self) -> usize {
        // Until FFmpeg's stdin isn't closed, we don't get the last few frames.
        // By supplying more than we need we can workaround this a bit.
        //
        // *: N is 16 for ffmpeg 7.1, tested on Mac & Windows. For ffmpeg 6.1.2 on Linux it was found to be 18.
        18
    }
}

fn write_bytes(stream: &mut dyn std::io::Write, data: &[u8]) -> Result<(), Error> {
    stream.write_all(data).map_err(Error::FailedToWriteToFfmpeg)
}

/// Ignore some common output from ffmpeg.
fn should_ignore_log_msg(msg: &str) -> bool {
    let patterns = [
        "Duration: N/A, bitrate: N/A",
        "frame=    0 fps=0.0 q=0.0 size=       0kB time=N/A bitrate=N/A speed=N/A",
        "encoder         : ", // Describes the encoder that was used to encode a video.
        "Metadata:",
        "Stream mapping:",
        // TODO(andreas): We see invalid IDR frames (NAL unit 5) on otherwise perfectly fine H265 video material
        // This one might be an actual bug! But so far no issues in playback have been observed.
        "Invalid NAL unit 5, skipping.",
        // It likes to say this a lot, almost no matter the format.
        // Some sources say this is more about internal formats, i.e. specific decoders using the wrong values, rather than the cli passed formats.
        "deprecated pixel format used, make sure you did set range correctly",
        // Not entirely sure why it tells us this sometimes:
        // Nowhere in the pipeline do we ask for this conversion, so it must be a transitional format?
        // This is supported by experimentation yielding that it shows only up when using the `-colorspace` parameter.
        // (color range and yuvj formats are fine though!)
        "No accelerated colorspace conversion found from yuv420p to bgr24",
        // We actually don't even want it to estimate a framerate!
        "not enough frames to estimate rate",
        // Similar: we don't want it to be able to estimate any of these things and we set those values explicitly, see invocation.
        // Observed on Windows FFmpeg 7.1, but not with the same version on Mac with the same video.
        "Consider increasing the value for the 'analyzeduration' (0) and 'probesize' (32) options",
        // Size etc. *is* specified in SPS & PPS, unclear why it's missing that.
        // Observed on Windows FFmpeg 7.1, but not with the same version on Mac with the same video.
        "Could not find codec parameters for stream 0 (Video: h264, none): unspecified size",
        // NOTE: We sometimes get a `[NULL @ 0x14f107150]`, which is not very actionable, but may be useful for debugging.
    ];

    // Why would we get an empty message? Observed on Windows FFmpeg 7.1.
    if msg.is_empty() {
        return true;
    }

    for pattern in patterns {
        if msg.contains(pattern) {
            return true;
        }
    }

    false
}

/// Strips out buffer addresses from `FFmpeg` log messages so that we can use it with the log-once family of methods.
fn sanitize_ffmpeg_log_message(msg: &str) -> String {
    // Make warn_once work on `[FOO @ 0x148db8000]` style warnings even if the address is different every time.
    // In older versions of FFmpeg this may happen several times in the same message (happens in 5.1, did not happen in 7.1).
    let mut msg = msg.to_owned();
    while let Some(start_pos) = msg.find(" @ 0x") {
        if let Some(end_offset) = msg[start_pos..].find(']') {
            if start_pos + end_offset + 1 > msg.len() {
                break;
            }

            msg = [&msg[..start_pos], &msg[start_pos + end_offset + 1..]].join("]");
        } else {
            // Huh, strange. Ignore it :shrug:
            break;
        }
    }

    msg
}

#[derive(Clone)]
enum CodecMeta {
    RawBytestream, // generic “pass-through” label for any format that’s ready to feed to the decoder as-is.
    Avc(re_mp4::Avc1Box),
    Hevc(re_mp4::HevcBox),
}

impl CodecMeta {
    fn from_stsd(stsd: &re_mp4::StsdBox) -> Option<Self> {
        use re_mp4::StsdBoxContent::{Avc1, Hev1, Hvc1};

        match &stsd.contents {
            Avc1(avc) => Some(Self::Avc(avc.clone())),
            Hev1(hevc) | Hvc1(hevc) => Some(Self::Hevc(hevc.clone())),

            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_ffmpeg_log_message;

    #[test]
    fn test_sanitize_ffmpeg_log_message() {
        assert_eq!(
            sanitize_ffmpeg_log_message("[swscaler @ 0x148db8000]"),
            "[swscaler]"
        );

        assert_eq!(
            sanitize_ffmpeg_log_message("[foo#0:0/h264 @ 0x148db8000]"),
            "[foo#0:0/h264]"
        );

        assert_eq!(
            sanitize_ffmpeg_log_message(
                "Some text prior [swscaler @ 0x148db8000] Warning: invalid pixel format specified"
            ),
            "Some text prior [swscaler] Warning: invalid pixel format specified"
        );

        assert_eq!(
            sanitize_ffmpeg_log_message(
                "Some text prior [swscaler @ 0x148db8000 other stuff we don't care about I guess] Warning: invalid pixel format specified"
            ),
            "Some text prior [swscaler] Warning: invalid pixel format specified"
        );

        assert_eq!(
            sanitize_ffmpeg_log_message(
                "[swscaler @ 0x148db8100] Warning: invalid poxel format specified [swscaler @ 0x148db8200]"
            ),
            "[swscaler] Warning: invalid poxel format specified [swscaler]"
        );

        assert_eq!(
            sanitize_ffmpeg_log_message(
                "[swscaler @ 0x248db8000] Warning: invalid päxel format specified [swscaler @ 0x198db8000] [swscaler @ 0x148db8030]"
            ),
            "[swscaler] Warning: invalid päxel format specified [swscaler] [swscaler]"
        );

        assert_eq!(
            sanitize_ffmpeg_log_message("[h264 @ 0x148db8000 something is wrong here"),
            "[h264 @ 0x148db8000 something is wrong here"
        );
        assert_eq!(
            sanitize_ffmpeg_log_message("h264 @ 0x148db8000] something is wrong here"),
            "h264] something is wrong here"
        );
    }
}
