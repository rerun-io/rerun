//! Transcode an mp4 through the `ffmpeg` CLI.
//!
//! ffmpeg reads the (seekable) source file directly — an mp4's `moov` sample
//! tables can trail its `mdat`, so a non-seekable stdin pipe can't be demuxed —
//! and writes a **fragmented** mp4 to stdout: `empty_moov` puts a demuxable init
//! segment up front, and `frag_keyframe` starts a new self-contained fragment at
//! each keyframe. [`TranscodedMp4`] yields those bytes as they are produced, so
//! nothing is buffered and the caller can demux one GOP at a time.

use std::collections::{BTreeSet, VecDeque};
use std::path::Path;

use ffmpeg_sidecar::child::FfmpegChild;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::{FfmpegEvent, LogLevel};
use ffmpeg_sidecar::iter::FfmpegIterator;

use super::FFmpegVersion;
use super::ffmpeg::{Error, check_ffmpeg_version};
use crate::{HwAccel, Mp4TranscodeOptions, VideoCodec};

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
            "ffmpeg exited with {status} while transcoding {debug_name}:\n{tail}",
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

/// Re-encode the mp4 at `input_path` into a B-frame-free, stream-friendly
/// fragmented mp4 (one `frag_keyframe` fragment per GOP, `empty_moov` init
/// segment up front), applying the transforms in `options`.
///
/// The re-encode is done at a visually-lossless quality and preserves the source pixel format where
/// possible, so the output is a faithful — not bit-exact — copy of the input.
///
/// Returns [`Error::FFmpegNotInstalled`] if no usable `ffmpeg` executable is
/// found, or [`Error::NoEncoderForCodec`] if this ffmpeg build has no encoder for
/// the requested output codec.
pub fn transcode_mp4(
    input_path: &Path,
    source_codec: VideoCodec,
    options: &Mp4TranscodeOptions,
    debug_name: &str,
) -> Result<TranscodedMp4, Error> {
    re_tracing::profile_function!();

    let ffmpeg_path = options.ffmpeg_override.as_deref();

    // Surfaces `Error::FFmpegNotInstalled` / `Error::UnsupportedFFmpegVersion`,
    // exactly like the decoder, and before the encoder probe below so a bogus
    // override fails here deterministically. Safe to block: we're off the GUI thread.
    check_ffmpeg_version(FFmpegVersion::for_executable_blocking(ffmpeg_path))?;

    let target = options.output_codec.clone().unwrap_or(source_codec);
    let available = available_encoders(ffmpeg_path);
    let spec = resolve_encoder(&target, options.hardware_acceleration, &available)?;

    let mut command = ffmpeg_command(ffmpeg_path);

    // ffmpeg seeks the source itself; no stdin piping (mp4 can't be demuxed from a pipe).
    command.input(input_path.to_string_lossy().as_ref());
    command.args(["-c:v", spec.name]);
    command.args(spec.rate_control.iter().map(String::as_str));
    if spec.needs_bf0 {
        // No B-frames in H.26x output: `VideoStream` can't model `DTS != PTS`
        // (#10090). AV1/VP8/VP9 are inherently `DTS == PTS`, so they don't need it.
        command.args(["-bf", "0"]);
    }
    if let Some(gop) = options.gop_size {
        // `-g` sets the max keyframe interval; `-force_key_frames` guarantees a
        // keyframe exactly every `gop` frames, codec-agnostically (unlike the
        // x264-only `-sc_threshold 0`), so the GOP length — and thus the per-GOP
        // fragmentation below — is deterministic.
        command.args(["-g", &gop.to_string()]);
        command.args(["-force_key_frames", &format!("expr:gte(n,n_forced*{gop})")]);
    }
    command.args(spec.extra_output_args.iter().map(String::as_str));

    let mut child = command
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

/// A resolved ffmpeg encoder plus the args needed to drive it for B-frame-free,
/// stream-friendly output.
struct EncoderSpec {
    /// The ffmpeg `-c:v` encoder name.
    name: &'static str,

    /// Rate-control args (e.g. `-crf 18`, or `-cq 23` for a GPU encoder).
    rate_control: Vec<String>,

    /// Extra codec/muxer args (e.g. `-strict experimental` so the mp4 muxer will
    /// write a VP8 track).
    extra_output_args: Vec<String>,

    /// Whether `-bf 0` must be passed. Only H.26x need it; AV1/VP8/VP9 are
    /// inherently `DTS == PTS`.
    needs_bf0: bool,
}

impl EncoderSpec {
    fn new(name: &'static str, rate_control: &[&str], needs_bf0: bool) -> Self {
        Self {
            name,
            rate_control: rate_control.iter().map(|s| (*s).to_owned()).collect(),
            extra_output_args: Vec::new(),
            needs_bf0,
        }
    }

    fn with_extra(mut self, extra: &[&str]) -> Self {
        self.extra_output_args = extra.iter().map(|s| (*s).to_owned()).collect();
        self
    }
}

/// Pick the ffmpeg encoder (and its rate-control flags) for `target`, preferring
/// a hardware encoder when `hw == Auto` and one is `available`, otherwise falling
/// back to software. Returns [`Error::NoEncoderForCodec`] if none is available.
///
/// This is the single maintenance point for encoder/flag choices — the quality
/// defaults and GPU flags are best-effort. Kept pure (takes the already-probed
/// `available` set) so it is unit-testable without spawning ffmpeg.
fn resolve_encoder(
    target: &VideoCodec,
    hw: HwAccel,
    available: &BTreeSet<String>,
) -> Result<EncoderSpec, Error> {
    // (gpu candidates in priority order, software candidates in priority order).
    // GPU is limited to the NVENC + VideoToolbox families (defined rate-control);
    // QSV/VAAPI are deferred. VP8/VP9's only GPU encoders are the deferred Intel
    // ones (`vp8_vaapi`/`vp9_vaapi`/`vp9_qsv`), so their GPU list is empty here.
    let (gpu, sw): (Vec<EncoderSpec>, Vec<EncoderSpec>) = match target {
        VideoCodec::H264 => (
            vec![
                EncoderSpec::new("h264_nvenc", &["-rc", "vbr", "-cq", "23"], true),
                EncoderSpec::new("h264_videotoolbox", &["-q:v", "55"], true),
            ],
            vec![EncoderSpec::new("libx264", &["-crf", "18"], true)],
        ),
        VideoCodec::H265 => (
            vec![
                EncoderSpec::new("hevc_nvenc", &["-rc", "vbr", "-cq", "23"], true),
                EncoderSpec::new("hevc_videotoolbox", &["-q:v", "55"], true),
            ],
            vec![EncoderSpec::new("libx265", &["-crf", "20"], true)],
        ),
        VideoCodec::AV1 => (
            vec![EncoderSpec::new(
                "av1_nvenc",
                &["-rc", "vbr", "-cq", "30"],
                false,
            )],
            vec![
                EncoderSpec::new("libsvtav1", &["-crf", "30"], false),
                EncoderSpec::new("libaom-av1", &["-crf", "30", "-b:v", "0"], false),
            ],
        ),
        VideoCodec::VP9 => (
            Vec::new(),
            // libvpx constant-quality needs `-b:v 0`.
            vec![EncoderSpec::new(
                "libvpx-vp9",
                &["-crf", "31", "-b:v", "0"],
                false,
            )],
        ),
        VideoCodec::VP8 => (
            Vec::new(),
            // VP8 CQ needs a bitrate cap, and the mp4 muxer needs `-strict experimental`.
            vec![
                EncoderSpec::new("libvpx", &["-crf", "10", "-b:v", "2M"], false)
                    .with_extra(&["-strict", "experimental"]),
            ],
        ),
        VideoCodec::ImageSequence(_) => {
            // The reader rejects an image-sequence target before we get here; this
            // is a defensive backstop.
            return Err(Error::BadVideoData(format!(
                "Cannot transcode to a non-video (image-sequence) codec {target:?}"
            )));
        }
    };

    if hw == HwAccel::Auto {
        if let Some(spec) = gpu.into_iter().find(|c| available.contains(c.name)) {
            return Ok(spec);
        }
        re_log::warn_once!(
            "No hardware encoder available in this FFmpeg build for {target:?}; using a software encoder"
        );
    }
    if let Some(spec) = sw.into_iter().find(|c| available.contains(c.name)) {
        return Ok(spec);
    }
    Err(Error::NoEncoderForCodec {
        codec: target.clone(),
    })
}

/// Build an [`FfmpegCommand`] for the given override (or `PATH`).
///
/// The single place the ffmpeg binary is resolved, so the encoder probe and the
/// transcode itself never end up pointing at different executables.
fn ffmpeg_command(ffmpeg_path: Option<&Path>) -> FfmpegCommand {
    match ffmpeg_path {
        Some(path) => FfmpegCommand::new_with_path(path),
        None => FfmpegCommand::new(),
    }
}

/// The set of encoder names this ffmpeg build reports via `ffmpeg -encoders`.
///
/// Returns an empty set if ffmpeg can't be run.
fn available_encoders(ffmpeg_path: Option<&Path>) -> BTreeSet<String> {
    // `new_with_path` already pipes stdout; silence stderr (the `-loglevel` line).
    let output = match ffmpeg_command(ffmpeg_path)
        .as_inner_mut()
        .args(["-hide_banner", "-encoders"])
        .stderr(std::process::Stdio::null())
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            re_log::warn_once!("Failed to probe FFmpeg encoders: {err}");
            return BTreeSet::new();
        }
    };
    parse_encoder_names(&String::from_utf8_lossy(&output.stdout))
}

/// Parse the encoder names out of `ffmpeg -encoders` stdout.
///
/// The listing is a header, a `------` separator, then one line per encoder:
/// a 6-char flag column (first char `V`/`A`/`S`) then the encoder name, e.g.
/// ` V....D h264_nvenc NVIDIA NVENC H.264 encoder`. We keep the names of the
/// video (`V`) encoders.
fn parse_encoder_names(stdout: &str) -> BTreeSet<String> {
    // Everything after the `------` separator (or the whole text if not found).
    let body = stdout.split_once("------").map_or(stdout, |(_, rest)| rest);
    body.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let flags = parts.next()?;
            (flags.len() == 6 && flags.starts_with('V'))
                .then(|| parts.next())
                .flatten()
                .map(str::to_owned)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{parse_encoder_names, resolve_encoder};
    use crate::{HwAccel, VideoCodec};

    fn available_set(names: &[&str]) -> std::collections::BTreeSet<String> {
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn software_specs_per_codec() {
        // Every software encoder present; assert the expected name, rate-control,
        // and whether `-bf 0` is needed.
        let all = available_set(&["libx264", "libx265", "libsvtav1", "libvpx-vp9", "libvpx"]);
        let cases = [
            (VideoCodec::H264, "libx264", vec!["-crf", "18"], true),
            (VideoCodec::H265, "libx265", vec!["-crf", "20"], true),
            (VideoCodec::AV1, "libsvtav1", vec!["-crf", "30"], false),
            (
                VideoCodec::VP9,
                "libvpx-vp9",
                vec!["-crf", "31", "-b:v", "0"],
                false,
            ),
            (
                VideoCodec::VP8,
                "libvpx",
                vec!["-crf", "10", "-b:v", "2M"],
                false,
            ),
        ];
        for (codec, name, rc, needs_bf0) in cases {
            let spec = resolve_encoder(&codec, HwAccel::Off, &all).expect("encoder available");
            assert_eq!(spec.name, name, "{codec:?}");
            assert_eq!(spec.rate_control, rc, "{codec:?}");
            assert_eq!(spec.needs_bf0, needs_bf0, "{codec:?}");
        }
    }

    #[test]
    fn vp8_carries_strict_experimental() {
        let spec = resolve_encoder(&VideoCodec::VP8, HwAccel::Off, &available_set(&["libvpx"]))
            .expect("libvpx available");
        assert_eq!(spec.extra_output_args, vec!["-strict", "experimental"]);
    }

    #[test]
    fn auto_prefers_hardware_then_falls_back_to_software() {
        // GPU present → picked.
        let with_gpu = available_set(&["h264_nvenc", "libx264"]);
        let spec = resolve_encoder(&VideoCodec::H264, HwAccel::Auto, &with_gpu).unwrap();
        assert_eq!(spec.name, "h264_nvenc");

        // GPU absent → software fallback.
        let sw_only = available_set(&["libx264"]);
        let spec = resolve_encoder(&VideoCodec::H264, HwAccel::Auto, &sw_only).unwrap();
        assert_eq!(spec.name, "libx264");
    }

    #[test]
    fn auto_for_codec_without_gpu_encoder_uses_software() {
        // VP9's only GPU encoders are the deferred Intel ones, so within scope it
        // has none; `Auto` must still resolve to software.
        let spec = resolve_encoder(
            &VideoCodec::VP9,
            HwAccel::Auto,
            &available_set(&["libvpx-vp9"]),
        )
        .unwrap();
        assert_eq!(spec.name, "libvpx-vp9");
    }

    #[test]
    fn av1_falls_back_to_libaom_when_svtav1_missing() {
        let spec = resolve_encoder(
            &VideoCodec::AV1,
            HwAccel::Off,
            &available_set(&["libaom-av1"]),
        )
        .unwrap();
        assert_eq!(spec.name, "libaom-av1");
        assert_eq!(spec.rate_control, vec!["-crf", "30", "-b:v", "0"]);
    }

    #[test]
    fn no_encoder_available_is_an_error() {
        let err = resolve_encoder(&VideoCodec::VP9, HwAccel::Off, &available_set(&[]));
        assert!(matches!(
            err,
            Err(super::Error::NoEncoderForCodec {
                codec: VideoCodec::VP9
            })
        ));
    }

    #[test]
    fn parse_encoder_names_extracts_video_encoders() {
        let sample = "\
Encoders:
 V..... = Video
 A..... = Audio
 ------
 V....D libx264              libx264 H.264 / AVC
 V....D h264_nvenc           NVIDIA NVENC H.264 encoder
 A....D aac                  AAC (Advanced Audio Coding)
 V....D libvpx-vp9           libvpx VP9
";
        let got = parse_encoder_names(sample);
        assert!(got.contains("libx264"));
        assert!(got.contains("h264_nvenc"));
        assert!(got.contains("libvpx-vp9"));
        // Audio encoders are not video, so they're excluded.
        assert!(!got.contains("aac"));
    }
}
