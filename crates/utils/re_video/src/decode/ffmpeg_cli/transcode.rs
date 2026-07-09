//! Transcode an mp4 through the `ffmpeg` CLI.
//!
//! ffmpeg reads the (seekable) source file directly — an mp4's `moov` sample
//! tables can trail its `mdat`, so a non-seekable stdin pipe can't be demuxed —
//! and writes a **fragmented** mp4 to stdout: `empty_moov` puts a demuxable init
//! segment up front, and `frag_keyframe` starts a new self-contained fragment at
//! each keyframe. [`TranscodedMp4`] yields those bytes as they are produced, so
//! nothing is buffered and the caller can demux one GOP at a time.

use std::collections::VecDeque;
use std::path::Path;

use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use ffmpeg_sidecar::iter::FfmpegIterator;

use super::FFmpegVersion;
use super::ffmpeg::{Error, check_ffmpeg_version};
use crate::VideoCodec;

/// How many trailing error/fatal stderr lines to keep for error reporting.
const STDERR_TAIL_LINES: usize = 40;

/// A streaming iterator over the transcoded, fragmented mp4 that `ffmpeg`
/// writes to stdout, yielded as raw byte chunks that do *not* align to mp4 box
/// or GOP boundaries (the caller must reframe them).
pub struct TranscodedMp4 {
    child: FfmpegChild,
    events: FfmpegIterator,

    /// Bounded tail of ffmpeg's error/fatal log lines, kept so a non-zero exit
    /// can report why it failed.
    stderr_tail: VecDeque<String>,

    debug_name: String,
    done: bool,
}

impl Iterator for TranscodedMp4 {
    /// One chunk of the fragmented-mp4 output, or a terminal error.
    type Item = Result<Vec<u8>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        // Drive the shared event stream until we have a chunk to hand back or the
        // process is done. Metadata, progress, and benign logs are skipped; the
        // internal reader thread blocks on its rendezvous channel between our
        // calls, so this keeps only one chunk resident at a time.
        loop {
            match self.events.next() {
                Some(FfmpegEvent::OutputChunk(chunk)) => return Some(Ok(chunk)),

                // ffmpeg's own diagnostics: keep a bounded tail of the serious
                // ones so a non-zero exit can explain itself.
                Some(FfmpegEvent::Log(LogLevel::Error | LogLevel::Fatal, line)) => {
                    self.push_tail(line);
                }

                // An error from `ffmpeg_sidecar` itself, rather than from ffmpeg.
                Some(FfmpegEvent::Error(err)) => {
                    self.done = true;
                    return Some(Err(Error::FfmpegSidecar(err)));
                }

                // stdout closed (`Done`) or both reader threads finished (`None`):
                // ffmpeg is done writing. Reap it and surface a non-zero exit.
                Some(FfmpegEvent::Done) | None => {
                    self.done = true;
                    return self.finish().err().map(Err);
                }

                // Metadata, progress, `LogEOF`, and benign logs: nothing to emit.
                Some(_) => {}
            }
        }
    }
}

impl TranscodedMp4 {
    fn push_tail(&mut self, line: String) {
        if self.stderr_tail.len() >= STDERR_TAIL_LINES {
            self.stderr_tail.pop_front();
        }
        self.stderr_tail.push_back(line);
    }

    /// Reap ffmpeg once its output has ended; `Err` if it exited non-zero.
    fn finish(&mut self) -> Result<(), Error> {
        let status = self
            .child
            .wait()
            .map_err(|err| Error::FfmpegSidecar(format!("failed to await ffmpeg: {err}")))?;
        if status.success() {
            return Ok(());
        }
        let tail = Vec::from(std::mem::take(&mut self.stderr_tail)).join("\n");
        Err(Error::Ffmpeg(format!(
            "ffmpeg exited with {status} while stripping B-frames from {debug_name}:\n{tail}",
            debug_name = self.debug_name,
        )))
    }
}

impl Drop for TranscodedMp4 {
    fn drop(&mut self) {
        if !self.done {
            // Consumer stopped early (or an error aborted us): kill ffmpeg so its
            // reader threads unblock and it doesn't linger on a full stdout pipe.
            // The threads are detached and exit on the resulting pipe EOF.
            self.child.kill().ok();
            self.child.wait().ok();
        }
    }
}

/// Re-encode the mp4 at `input_path`, dropping all B-frames (`-bf 0`).
///
/// Keeps the same video codec and streams the result out as a fragmented mp4
/// (one `frag_keyframe` fragment per GOP, `empty_moov` init segment up front).
///
/// The re-encode is unavoidable (dropping B-frames changes the GOP reference
/// structure), but is done at a visually-lossless CRF and preserves the source
/// pixel format, so the output is a faithful, similarly-sized copy of the input
/// rather than a bit-exact one.
///
/// Returns [`Error::FFmpegNotInstalled`] (with the same message the decoder
/// surfaces) if no usable `ffmpeg` executable is found at `ffmpeg_path` (or on
/// `PATH` when `ffmpeg_path` is `None`).
pub fn transcode_mp4_drop_b_frames(
    input_path: &Path,
    codec: VideoCodec,
    ffmpeg_path: Option<&Path>,
    debug_name: &str,
) -> Result<TranscodedMp4, Error> {
    re_tracing::profile_function!();

    // Surfaces `Error::FFmpegNotInstalled` / `Error::UnsupportedFFmpegVersion`,
    // exactly like the decoder. Safe to block here: we're already off the GUI thread.
    check_ffmpeg_version(FFmpegVersion::for_executable_blocking(ffmpeg_path))?;

    // Keep the source codec so the resulting `VideoStream` matches the input.
    //
    // We must re-encode (dropping B-frames changes the GOP reference structure, so
    // bit-identical output is impossible), so pick a visually-lossless CRF: the
    // output is perceptually indistinguishable from the source while staying close
    // to its size. x265's CRF scale runs ~2 lower than x264's for equivalent
    // quality, hence the different value.
    let (encoder, crf) = match codec {
        VideoCodec::H264 => ("libx264", "18"),
        VideoCodec::H265 => ("libx265", "20"),
        other => {
            return Err(Error::BadVideoData(format!(
                "Cannot strip B-frames from a {other:?} stream: \
                 only H.264 and H.265 re-encoding is supported"
            )));
        }
    };

    let mut command = if let Some(ffmpeg_path) = ffmpeg_path {
        FfmpegCommand::new_with_path(ffmpeg_path)
    } else {
        FfmpegCommand::new()
    };

    let mut child = command
        // ffmpeg seeks the source itself; no stdin piping (mp4 can't be demuxed from a pipe).
        .input(input_path.to_string_lossy().as_ref())
        .args(["-c:v", encoder])
        .args(["-bf", "0"]) // the whole point: no B-frames in the output.
        .args(["-crf", crf]) // visually lossless: faithful to the source, similar size.
        // Deliberately no `-pix_fmt`: ffmpeg then negotiates the encoder's format from
        // the decoded input, preserving the source bit depth / chroma subsampling
        // (10-bit, 4:2:2, 4:4:4) instead of forcing a downconvert to 8-bit 4:2:0.
        .fps_mode("passthrough") // keep every frame (no dupes/drops), at its original PTS.
        .args(["-an"]) // `VideoStream` carries no audio.
        // Fragmented mp4: `empty_moov` puts a demuxable init segment up front and
        // `frag_keyframe` starts a new self-contained fragment at each keyframe, so
        // the output is streamable and splits cleanly into one GOP per fragment.
        .args(["-movflags", "frag_keyframe+empty_moov+default_base_moof"])
        .args(["-f", "mp4"])
        .output("pipe:1")
        .spawn()
        .map_err(Error::FailedToStartFfmpeg)?;

    // The same event iterator the decoder consumes: it spawns the stdout/stderr
    // reader threads internally and delivers their output over a rendezvous
    // channel, so we neither manage threads nor buffer the whole stream.
    let events = child
        .iter()
        .map_err(|err| Error::NoIterator(err.to_string()))?;

    Ok(TranscodedMp4 {
        child,
        events,
        stderr_tail: VecDeque::with_capacity(STDERR_TAIL_LINES),
        debug_name: debug_name.to_owned(),
        done: false,
    })
}
