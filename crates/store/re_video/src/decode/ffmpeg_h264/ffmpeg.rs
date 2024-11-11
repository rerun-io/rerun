//! Send video data to `ffmpeg` over CLI to decode it.

use std::{
    collections::BTreeMap,
    process::ChildStdin,
    sync::{atomic::AtomicBool, Arc},
};

use crossbeam::channel::{Receiver, Sender};
use ffmpeg_sidecar::{
    child::FfmpegChild,
    command::FfmpegCommand,
    event::{FfmpegEvent, LogLevel},
};
use parking_lot::Mutex;

use crate::{
    decode::{
        ffmpeg_h264::{
            nalu::{NalHeader, NalUnitType, NAL_START_CODE},
            sps::H264Sps,
        },
        AsyncDecoder, Chunk, Frame, FrameContent, FrameInfo, OutputCallback,
    },
    PixelFormat, Time,
};

// FFmpeg 5.1 "Riemann" is from 2022-07-22.
// It's simply the oldest I tested manually as of writing. We might be able to go lower.
const FFMPEG_MINIMUM_VERSION_MAJOR: u32 = 5;
const FFMPEG_MINIMUM_VERSION_MINOR: u32 = 1;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Couldn't find an installation of the FFmpeg executable.")]
    FfmpegNotInstalled {
        /// Download URL for the latest version of `FFmpeg` on the current platform.
        /// None if the platform is not supported.
        // TODO(andreas): as of writing, ffmpeg-sidecar doesn't define a download URL for linux arm.
        download_url: Option<&'static str>,
    },

    #[error("Failed to start FFmpeg: {0}")]
    FailedToStartFfmpeg(std::io::Error),

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

impl From<Error> for crate::decode::Error {
    fn from(err: Error) -> Self {
        if let Error::FfmpegNotInstalled { download_url } = err {
            Self::FfmpegNotInstalled { download_url }
        } else {
            Self::Ffmpeg(std::sync::Arc::new(err))
        }
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
        if self.shutdown.load(std::sync::atomic::Ordering::Acquire) {
            Err(std::io::Error::new(Self::SHUTDOWN_ERROR_KIND, "shutdown"))
        } else {
            self.stdin.write(buf)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.shutdown.load(std::sync::atomic::Ordering::Acquire) {
            Err(std::io::Error::new(Self::SHUTDOWN_ERROR_KIND, "shutdown"))
        } else {
            self.stdin.flush()
        }
    }
}

struct FfmpegProcessAndListener {
    ffmpeg: FfmpegChild,

    /// For sending frame timestamps to the ffmpeg listener thread.
    frame_info_tx: Sender<FfmpegFrameInfo>,

    /// For sending chunks to the ffmpeg write thread.
    frame_data_tx: Sender<FfmpegFrameData>,

    listen_thread: Option<std::thread::JoinHandle<()>>,
    write_thread: Option<std::thread::JoinHandle<()>>,

    /// If true, the write thread will not report errors. Used upon exit, so the write thread won't log spam on the hung up stdin.
    stdin_shutdown: Arc<AtomicBool>,

    /// On output instance used by the threads.
    on_output: Arc<Mutex<Option<Arc<OutputCallback>>>>,
}

impl FfmpegProcessAndListener {
    fn new(
        debug_name: &str,
        on_output: Arc<OutputCallback>,
        avcc: re_mp4::Avc1Box,
    ) -> Result<Self, Error> {
        re_tracing::profile_function!();

        if !ffmpeg_sidecar::command::ffmpeg_is_installed() {
            return Err(Error::FfmpegNotInstalled {
                download_url: ffmpeg_sidecar::download::ffmpeg_download_url().ok(),
            });
        }

        let sps_result = H264Sps::parse_from_avcc(&avcc);
        if let Ok(sps) = &sps_result {
            re_log::trace!("Successfully parsed SPS for {debug_name}:\n{sps:?}");
        }

        let (pixel_format, ffmpeg_pix_fmt) = match sps_result.and_then(|sps| sps.pixel_layout()) {
            Ok(layout) => {
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
                let ffmpeg_pix_fmt = match layout {
                    crate::decode::YuvPixelLayout::Y_U_V444 => "yuvj444p",
                    crate::decode::YuvPixelLayout::Y_U_V422 => "yuvj422p",
                    crate::decode::YuvPixelLayout::Y_U_V420 => "yuvj420p",
                    crate::decode::YuvPixelLayout::Y400 => "gray",
                };

                (pixel_format, ffmpeg_pix_fmt)
            }
            Err(err) => {
                re_log::warn_once!(
                    "Failed to parse sequence parameter set (SPS) for {debug_name}: {err}"
                );

                // By default play it safe: let ffmpeg convert to rgba.
                (PixelFormat::Rgba8Unorm, "rgba")
            }
        };

        let mut ffmpeg = FfmpegCommand::new()
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
            .format("h264") // TODO(andreas): should we check ahead of time whether this is available?
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

        let stdin_shutdown = Arc::new(AtomicBool::new(false));

        // Mutex protect `on_output` so that we can shut down the threads at a defined point in time at which we
        // no longer receive any new frames or errors from this process.
        let on_output = Arc::new(Mutex::new(Some(on_output)));

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
                        &pixel_format,
                        on_output.as_ref(),
                    );
                }
            })
            .expect("Failed to spawn ffmpeg listener thread");
        let write_thread = std::thread::Builder::new()
            .name(format!("ffmpeg-writer for {debug_name}"))
            .spawn({
                let on_output = on_output.clone();
                let ffmpeg_stdin = ffmpeg.take_stdin().ok_or(Error::NoStdin)?;
                let mut ffmpeg_stdin = StdinWithShutdown {
                    stdin: ffmpeg_stdin,
                    shutdown: stdin_shutdown.clone(),
                };
                move || {
                    write_ffmpeg_input(
                        &mut ffmpeg_stdin,
                        &frame_data_rx,
                        on_output.as_ref(),
                        &avcc,
                    );
                }
            })
            .expect("Failed to spawn ffmpeg writer thread");

        Ok(Self {
            ffmpeg,
            frame_info_tx,
            frame_data_tx,
            listen_thread: Some(listen_thread),
            write_thread: Some(write_thread),
            stdin_shutdown,
            on_output,
        })
    }
}

impl Drop for FfmpegProcessAndListener {
    fn drop(&mut self) {
        re_tracing::profile_function!();

        // Stop all outputs from being written to - any attempt from here on out will fail and cause thread shutdown.
        // This way, we ensure all ongoing writes are finished and won't get any more on_output callbacks from this process
        // before we take any other action on the shutdown sequence.
        {
            self.on_output.lock().take();
        }

        // Notify (potentially wake up) the stdin write thread to stop it (it might be sleeping).
        self.frame_data_tx.send(FfmpegFrameData::EndOfStream).ok();
        // Kill stdin for the write thread. This helps cancelling ongoing stream write operations.
        self.stdin_shutdown
            .store(true, std::sync::atomic::Ordering::Release);

        // Kill the ffmpeg process itself.
        // It's important that we wait for it to finish, otherwise the process may enter a zombie state, see https://en.wikipedia.org/wiki/Zombie_process.
        // Also, a nice side effect of wait is that it ensures that stdin is closed.
        // This should wake up the listen thread if it is sleeping, but that may take a while.
        {
            let kill_result = self.ffmpeg.kill();
            let wait_result = self.ffmpeg.wait();
            re_log::debug!(
                "FFmpeg kill result: {:?}, wait result: {:?}",
                kill_result,
                wait_result
            );
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
                if let Some(write_thread) = self.write_thread.take() {
                    if write_thread.join().is_err() {
                        re_log::error!("Failed to join ffmpeg listener thread.");
                    }
                }
            }
            {
                re_tracing::profile_scope!("shutdown listen thread");
                if let Some(listen_thread) = self.listen_thread.take() {
                    if listen_thread.join().is_err() {
                        re_log::error!("Failed to join ffmpeg listener thread.");
                    }
                }
            }
        }
    }
}

fn write_ffmpeg_input(
    ffmpeg_stdin: &mut dyn std::io::Write,
    frame_data_rx: &Receiver<FfmpegFrameData>,
    on_output: &Mutex<Option<Arc<OutputCallback>>>,
    avcc: &re_mp4::Avc1Box,
) {
    let mut state = NaluStreamState::default();

    while let Ok(data) = frame_data_rx.recv() {
        let chunk = match data {
            FfmpegFrameData::Chunk(chunk) => chunk,
            FfmpegFrameData::EndOfStream => break,
        };

        if let Err(err) = write_avc_chunk_to_nalu_stream(avcc, ffmpeg_stdin, &chunk, &mut state) {
            let on_output = on_output.lock();
            if let Some(on_output) = on_output.as_ref() {
                let write_error = matches!(err, Error::FailedToWriteToFfmpeg(_));
                on_output(Err(err.into()));

                if write_error {
                    // This is unlikely to improve! Ffmpeg process likely died.
                    // By exiting here we hang up on the channel, making future attempts to push into it fail which should cause a reset eventually.
                    return;
                }
            } else {
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
    pixel_format: &PixelFormat,
    on_output: &Mutex<Option<Arc<OutputCallback>>>,
) -> Option<()> {
    // Pending frames, sorted by their presentation timestamp.
    let mut pending_frame_infos = BTreeMap::new();
    let mut highest_dts = Time::MIN; // Highest dts encountered so far.

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
                    re_log::warn_once!(
                        "{debug_name} decoder: {}",
                        sanitize_ffmpeg_log_message(&msg)
                    );
                }
            }

            FfmpegEvent::Log(LogLevel::Error, msg) => {
                (on_output.lock().as_ref()?)(Err(Error::Ffmpeg(msg).into()));
            }

            FfmpegEvent::Log(LogLevel::Fatal, msg) => {
                (on_output.lock().as_ref()?)(Err(Error::FfmpegFatal(msg).into()));
            }

            FfmpegEvent::Log(LogLevel::Unknown, msg) => {
                if msg.contains("system signals, hard exiting") {
                    // That was probably us, killing the process.
                    re_log::debug!("FFmpeg process for {debug_name} was killed");
                    return None;
                }
                if !should_ignore_log_msg(&msg) {
                    // Note that older ffmpeg versions don't flag their warnings as such and may end up here.
                    re_log::warn_once!(
                        "{debug_name} decoder: {}",
                        sanitize_ffmpeg_log_message(&msg)
                    );
                }
            }

            FfmpegEvent::LogEOF => {
                // This event proceeds `FfmpegEvent::Done`.
                // This happens on `pkill ffmpeg`, for instance.
            }

            FfmpegEvent::Error(error) => {
                // An error in ffmpeg sidecar itself, rather than ffmpeg.
                (on_output.lock().as_ref()?)(Err(Error::FfmpegSidecar(error).into()));
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

            FfmpegEvent::OutputFrame(frame) => {
                // We input frames into ffmpeg in decode (DTS) order, and so that's
                // also the order we will receive the `FrameInfo`s from `frame_info_rx`.
                // However, `ffmpeg` will re-order the frames to output them in presentation (PTS) order.
                // We want to accurately match the `FrameInfo` with its corresponding output frame.
                // To do that, we need to buffer frames that come out of ffmpeg.
                //
                // How do we know how large this buffer needs to be?
                // Whenever the highest known DTS is behind the PTS, we need to wait until the DTS catches up.
                // Otherwise, we'd assign the wrong PTS to the frame that just came in.
                let frame_info = loop {
                    let oldest_pts_in_buffer =
                        pending_frame_infos.first_key_value().map(|(pts, _)| *pts);
                    let is_caught_up = oldest_pts_in_buffer.is_some_and(|pts| pts <= highest_dts);
                    if is_caught_up {
                        // There must be an element here, otherwise we wouldn't be here.
                        #[allow(clippy::unwrap_used)]
                        break pending_frame_infos.pop_first().unwrap().1;
                    } else {
                        // We're behind:
                        let Ok(frame_info) = frame_info_rx.try_recv() else {
                            re_log::debug!(
                                "{debug_name} ffmpeg decoder frame info channel disconnected"
                            );
                            return None;
                        };

                        // If the decodetimestamp did not increase, we're probably seeking backwards!
                        // We'd expect the video player to do a reset prior to that and close the channel as part of that, but we may not have noticed that in here yet!
                        // In any case, we'll have to just run with this as the new highest timestamp, not much else we can do.
                        if highest_dts > frame_info.decode_timestamp {
                            re_log::warn!("Video decode timestamps are expected to monotonically increase unless there was a decoder reset.\n\
                                                It went from {:?} to {:?} for the decoder of {debug_name}. This is probably a bug in Rerun.", highest_dts, frame_info.decode_timestamp);
                        }
                        highest_dts = frame_info.decode_timestamp;

                        pending_frame_infos.insert(frame_info.presentation_timestamp, frame_info);
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

                debug_assert_eq!(
                    data.len() * 8,
                    (width * height * pixel_format.bits_per_pixel()) as usize
                );

                (on_output.lock().as_ref()?)(Ok(Frame {
                    content: FrameContent {
                        data,
                        width,
                        height,
                        format: pixel_format.clone(),
                    },
                    info: FrameInfo {
                        presentation_timestamp: frame_info.presentation_timestamp,
                        duration: frame_info.duration,
                        latest_decode_timestamp: Some(frame_info.decode_timestamp),
                    },
                }));
            }

            FfmpegEvent::Done => {
                // This happens on `pkill ffmpeg`, for instance.
                re_log::debug!("{debug_name}'s ffmpeg is Done");
                return None;
            }

            FfmpegEvent::ParsedVersion(ffmpeg_version) => {
                re_log::debug_once!("FFmpeg version is {}", ffmpeg_version.version);

                fn download_advice() -> String {
                    if let Ok(download_url) = ffmpeg_sidecar::download::ffmpeg_download_url() {
                        format!("\nYou can download an up to date version for your system at {download_url}.")
                    } else {
                        String::new()
                    }
                }

                // Version strings can get pretty wild!
                // E.g. choco installed ffmpeg on Windows gives me "7.1-essentials_build-www.gyan.dev".
                let mut version_parts = ffmpeg_version.version.split('.');
                let major = version_parts
                    .next()
                    .and_then(|part| part.parse::<u32>().ok());
                let minor = version_parts.next().and_then(|part| {
                    part.split('-')
                        .next()
                        .and_then(|part| part.parse::<u32>().ok())
                });

                if let (Some(major), Some(minor)) = (major, minor) {
                    re_log::debug_once!("Parsed FFmpeg version as {}.{}", major, minor);

                    if major < FFMPEG_MINIMUM_VERSION_MAJOR
                        || (major == FFMPEG_MINIMUM_VERSION_MAJOR
                            && minor < FFMPEG_MINIMUM_VERSION_MINOR)
                    {
                        re_log::warn_once!(
                            "FFmpeg version is {}. Only versions >= {FFMPEG_MINIMUM_VERSION_MAJOR}.{FFMPEG_MINIMUM_VERSION_MINOR} are officially supported.{}",
                            ffmpeg_version.version, download_advice()
                        );
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
                (on_output.lock().as_ref()?)(Err(Error::UnexpectedFfmpegOutputChunk.into()));
                return None;
            }
        }
    }

    Some(())
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
        on_output: impl Fn(crate::decode::Result<Frame>) + Send + Sync + 'static,
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
    fn submit_chunk(&mut self, chunk: Chunk) -> crate::decode::Result<()> {
        re_tracing::profile_function!();

        // We send the information about this chunk first.
        // Chunks are defined to always yield a single frame.
        let frame_info = FfmpegFrameInfo {
            presentation_timestamp: chunk.presentation_timestamp,
            decode_timestamp: chunk.decode_timestamp,
            duration: chunk.duration,
        };
        let chunk = FfmpegFrameData::Chunk(chunk);

        if self.ffmpeg.frame_info_tx.send(frame_info).is_err()
            || self.ffmpeg.frame_data_tx.send(chunk).is_err()
        {
            let err: crate::decode::Error =
                if let Ok(exit_code) = self.ffmpeg.ffmpeg.as_inner_mut().try_wait() {
                    Error::FfmpegUnexpectedExit(exit_code)
                } else {
                    Error::BrokenFrameInfoChannel
                }
                .into();

            // Report the error on the decoding stream.
            (self.on_output)(Err(err.clone()));
            return Err(err);
        }

        Ok(())
    }

    fn reset(&mut self) -> crate::decode::Result<()> {
        re_log::debug!("Resetting ffmpeg decoder {}", self.debug_name);
        self.ffmpeg = FfmpegProcessAndListener::new(
            &self.debug_name,
            self.on_output.clone(),
            self.avcc.clone(),
        )?;
        Ok(())
    }
}

#[derive(Default)]
struct NaluStreamState {
    previous_frame_was_idr: bool,
}

fn write_bytes(stream: &mut dyn std::io::Write, data: &[u8]) -> Result<(), Error> {
    stream.write_all(data).map_err(Error::FailedToWriteToFfmpeg)
}

fn write_avc_chunk_to_nalu_stream(
    avcc: &re_mp4::Avc1Box,
    nalu_stream: &mut dyn std::io::Write,
    chunk: &Chunk,
    state: &mut NaluStreamState,
) -> Result<(), Error> {
    re_tracing::profile_function!();
    let avcc = &avcc.avcc;

    // We expect the stream of chunks to not have any SPS (Sequence Parameter Set) & PPS (Picture Parameter Set)
    // just as it is the case with MP4 data.
    // In order to have every IDR frame be able to be fully re-entrant, we need to prepend the SPS & PPS NAL units.
    // Otherwise the decoder is not able to get the necessary information about how the video stream is encoded.
    if chunk.is_sync && !state.previous_frame_was_idr {
        for sps in &avcc.sequence_parameter_sets {
            write_bytes(nalu_stream, NAL_START_CODE)?;
            write_bytes(nalu_stream, &sps.bytes)?;
        }
        for pps in &avcc.picture_parameter_sets {
            write_bytes(nalu_stream, NAL_START_CODE)?;
            write_bytes(nalu_stream, &pps.bytes)?;
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

        // Can be useful for finding issues, but naturally very spammy.
        // let nal_header = NalHeader(chunk.data[data_start]);
        // re_log::trace!(
        //     "nal_header: {:?}, {}",
        //     nal_header.unit_type(),
        //     nal_header.ref_idc()
        // );

        let data = &chunk.data[data_start..data_end];

        write_bytes(nalu_stream, NAL_START_CODE)?;

        // Note that we don't have to insert "emulation prevention bytes" since mp4 NALU still use them.
        // (unlike the NAL start code, the presentation bytes are part of the NAL spec!)

        re_tracing::profile_scope!("write_bytes", data.len().to_string());
        write_bytes(nalu_stream, data)?;

        buffer_offset = data_end;
    }

    // Write an Access Unit Delimiter (AUD) NAL unit to the stream to signal the end of an access unit.
    // This can help with ffmpeg picking up NALs right away before seeing the next chunk.
    write_bytes(nalu_stream, NAL_START_CODE)?;
    write_bytes(
        nalu_stream,
        &[
            NalHeader::new(NalUnitType::AccessUnitDelimiter, 3).0,
            // Two arbitrary bytes? 0000 worked as well, but this is what
            // https://stackoverflow.com/a/44394025/ uses. Couldn't figure out the rules for this.
            0xFF,
            0x80,
        ],
    )?;

    Ok(())
}

/// Ignore some common output from ffmpeg.
fn should_ignore_log_msg(msg: &str) -> bool {
    let patterns = [
        "Duration: N/A, bitrate: N/A",
        "frame=    0 fps=0.0 q=0.0 size=       0kB time=N/A bitrate=N/A speed=N/A",
        "encoder         : ", // Describes the encoder that was used to encode a video.
        "Metadata:",
        "Stream mapping:",
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
    // Make warn_once work on `[swscaler @ 0x148db8000]` style warnings even if the address is different every time.
    // In older versions of FFmpeg this may happen several times in the same message (happens in 5.1, did not happen in 7.1).
    let mut msg = msg.to_owned();
    while let Some(start_pos) = msg.find("[swscaler @ 0x") {
        if let Some(end_offset) = msg[start_pos..].find(']') {
            if start_pos + end_offset + 1 > msg.len() {
                break;
            }

            msg = [&msg[..start_pos], &msg[start_pos + end_offset + 1..]].join("[swscaler]");
        } else {
            // Huh, strange. Ignore it :shrug:
            break;
        }
    }

    msg
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
            sanitize_ffmpeg_log_message("[swscaler @ 0x148db8100] Warning: invalid poxel format specified [swscaler @ 0x148db8200]"),
            "[swscaler] Warning: invalid poxel format specified [swscaler]"
        );

        assert_eq!(
            sanitize_ffmpeg_log_message("[swscaler @ 0x248db8000] Warning: invalid päxel format specified [swscaler @ 0x198db8000] [swscaler @ 0x148db8030]"),
            "[swscaler] Warning: invalid päxel format specified [swscaler] [swscaler]"
        );

        assert_eq!(
            sanitize_ffmpeg_log_message("[swscaler @ 0x148db8000 something is wrong here"),
            "[swscaler @ 0x148db8000 something is wrong here"
        );
        assert_eq!(
            sanitize_ffmpeg_log_message("swscaler @ 0x148db8000] something is wrong here"),
            "swscaler @ 0x148db8000] something is wrong here"
        );
    }
}
