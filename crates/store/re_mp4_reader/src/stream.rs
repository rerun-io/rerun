//! Stream-mode chunk emission: demux the mp4 with `re_video` and emit the
//! chunks described by [`crate::Mode::Stream`].
//!
//! `IsKeyframe` gets its own chunk because that is what `re_chunk_store`'s GOP
//! rebatching accepts — it rejects `is_keyframe=false` rows — and it keeps
//! keyframe queries off the sample column.
//!
//! Both modes reduce to the same pipeline — *emit the codec chunk, then turn a
//! sequence of demuxed [`Segment`]s into GOP chunks*:
//! - no B-frames: one segment, the source itself, emitted directly;
//! - B-frames: `VideoStream` can't model DTS != PTS, so ffmpeg re-encodes and
//!   streams back a fragmented mp4, and each `moof` fragment becomes one segment.
//!   Only one GOP is resident at a time.

use std::io::{Read, Seek, SeekFrom};

use itertools::Either;

use re_chunk::{Chunk, ChunkId, EntityPath, RowId, TimeColumn, TimePoint};
use re_log_types::{TimeType, Timeline, TimelineName};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::VideoCodec;
use re_span::Span;
use re_video::player::GetVideoSource;
use re_video::{
    Mp4TranscodeOptions, SampleIndex, SampleMetadataState, Time, TimeWindow, VideoDataDescription,
    VideoSource,
};

use crate::Mp4Error;

/// A `Read + Seek` source, type-erased so a segment can wrap either a file, an
/// in-memory buffer, or a transcoded mini-mp4.
trait ReadSeek: Read + Seek {}

impl<T: Read + Seek> ReadSeek for T {}

type ChunkIter = Box<dyn Iterator<Item = Result<Chunk, Mp4Error>>>;

/// The mp4 source for stream mode.
///
/// Kept as an owned handle (rather than an already-opened reader) so that, if a
/// transcode is required, we can hand ffmpeg a seekable *path* — an mp4's `moov`
/// sample tables can trail its `mdat`, so a non-seekable stdin pipe can't be
/// demuxed.
pub(crate) enum StreamInput {
    /// A file on disk.
    #[cfg(not(target_arch = "wasm32"))]
    Path(std::path::PathBuf),

    /// In-memory bytes.
    Bytes(Vec<u8>),
}

impl StreamInput {
    /// Open a fresh reader over the source.
    // On wasm the only variant is `Bytes` (an infallible `Cursor`), so the
    // `Result` looks redundant there — but it's needed for `File::open` natively.
    #[cfg_attr(target_arch = "wasm32", expect(clippy::unnecessary_wraps))]
    fn open(&self) -> Result<Box<dyn ReadSeek>, Mp4Error> {
        Ok(match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Path(path) => Box::new(std::io::BufReader::new(std::fs::File::open(path)?)),
            Self::Bytes(bytes) => Box::new(std::io::Cursor::new(bytes.clone())),
        })
    }
}

/// The chunk-emission parameters — constant across every chunk of one stream read.
#[derive(Clone)]
pub(crate) struct Emission {
    pub entity_path: EntityPath,
    pub timeline_name: TimelineName,
    pub timeline_type: TimeType,
    pub chunk_by_gop: bool,
}

/// Build a chunk iterator for stream mode.
///
/// Demuxes the source once to inspect it, then emits the static codec chunk,
/// the GOP chunks of each [`Segment`], and finally the `IsKeyframe` marker chunk.
/// The image-sequence-codec, keyframe, and timescale checks are performed eagerly
/// (here and in [`Segment::new`]) so callers see those errors from this
/// constructor rather than from the first `.next()` on the iterator.
pub(crate) fn iter_chunks(
    input: StreamInput,
    emission: Emission,
    transcode: &Mp4TranscodeOptions,
    time_window: Option<TimeWindow>,
    debug_name: &str,
) -> Result<ChunkIter, Mp4Error> {
    re_tracing::profile_function!();

    let mut reader = input.open()?;
    let size = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;
    let desc = VideoDataDescription::load_mp4_from_reader(&mut reader, size, debug_name)?;

    // Reject an image-sequence *source*: `Mode::Stream` can't represent it.
    // (`components::VideoCodec` has exactly the five real codecs; `try_from` errors
    // only for image sequences.)
    VideoCodec::try_from(desc.codec.clone()).map_err(|_err| Mp4Error::ImageSequenceInStreamMode)?;

    // The emitted `VideoStream` codec follows the *output* codec — the requested
    // `output_codec`, or the source codec when none is requested. This also rejects
    // an image-sequence *target*.
    let target_codec = transcode
        .output_codec
        .clone()
        .unwrap_or_else(|| desc.codec.clone());
    let output_mapped_codec =
        VideoCodec::try_from(target_codec).map_err(|_err| Mp4Error::ImageSequenceInStreamMode)?;

    // A re-encode is forced by container-level B-frame reordering (which only
    // H.264/H.265 produce, and which `VideoStream` can't model — #10090) or by a
    // requested transform (a *different* output codec, or a GOP size). Requesting
    // the output codec the source already uses is a no-op and stays direct.
    let needs_decoder_reordering = !desc.samples_statistics.dts_always_equal_pts;
    let needs_reencode = needs_decoder_reordering || transcode.requests_transform(&desc.codec);

    // Each segment carries its own cut: the transcode path needs none (ffmpeg's
    // output is already window-relative and frame-exact), the direct path slices
    // with a [`WindowCut`], and a window with a mid-GOP start mixes the two
    // (see [`WindowRoute`]).
    type Segments = Box<dyn Iterator<Item = Result<(Segment, Option<WindowCut>), Mp4Error>>>;

    let segments: Segments = if needs_reencode {
        // ffmpeg re-encodes (applying any window itself) and streams back a
        // fragmented mp4, one segment per GOP fragment. Only one GOP is resident
        // at a time.
        drop(reader);
        cfg_select! {
            target_arch = "wasm32" => {
                let _ = (input, transcode, time_window);
                return Err(Mp4Error::TranscodeRequiresFfmpeg);
            }
            _ => {
                Box::new(
                    transcoded_segments(
                        input,
                        desc.codec.clone(),
                        transcode,
                        time_window,
                        debug_name,
                    )?
                    .map(|segment| segment.map(|s| (s, None))),
                )
            }
        }
    } else {
        match time_window
            .map(|w| WindowRoute::resolve(&desc, w))
            .transpose()?
        {
            // No window: read the source in place (sample bytes are fetched on
            // demand, so the whole file is never resident).
            None => Box::new(std::iter::once(
                Segment::new(reader, desc).map(|s| (s, None)),
            )),

            // The whole window is copied directly, sliced by the cut.
            Some(WindowRoute::Direct(cut)) => Box::new(std::iter::once(
                Segment::new(reader, desc).map(|s| (s, Some(cut))),
            )),

            // Smart cut the time window using ffmpeg; transcode the first GOP, copy
            // the rest.
            Some(WindowRoute::Split {
                head,
                head_cut,
                tail_cut,
            }) => {
                cfg_select! {
                    target_arch = "wasm32" => {
                        let _ = (input, transcode, head, head_cut, tail_cut);
                        drop(reader);
                        return Err(Mp4Error::TranscodeRequiresFfmpeg);
                    }
                    _ => {
                        // Like the full transcode: ffmpeg needs a seekable file.
                        let StreamInput::Path(path) = &input else {
                            return Err(Mp4Error::TranscodeRequiresSeekableFile);
                        };
                        let head_segments = transcoded_segments(
                            StreamInput::Path(path.clone()),
                            desc.codec.clone(),
                            transcode,
                            Some(head),
                            debug_name,
                        )?
                        .map(move |segment| segment.map(|s| (s, Some(head_cut))));

                        // A `None` tail cut means the window ends inside the
                        // split GOP: the head is the whole window.
                        let tail: Segments = if let Some(cut) = tail_cut {
                            Box::new(std::iter::once(
                                Segment::new(reader, desc).map(|s| (s, Some(cut))),
                            ))
                        } else {
                            drop(reader);
                            Box::new(std::iter::empty())
                        };
                        Box::new(std::iter::chain(head_segments, tail))
                    }
                }
            }
        }
    };

    let codec_chunk = build_codec_chunk(&emission.entity_path, output_mapped_codec);

    let sample_chunks = {
        let emission = emission.clone();
        segments.flat_map(move |item| gop_chunks(item, emission.clone()))
    };

    Ok(Box::new(std::iter::chain(
        std::iter::once(codec_chunk),
        WithKeyframeMarker {
            sample_chunks,
            entity_path: emission.entity_path,
            timeline_name: emission.timeline_name,
            timeline_type: emission.timeline_type,
            keyframe_times: Vec::new(),
            done: false,
        },
    )))
}

/// Passes the sample chunks through, then yields the `IsKeyframe` marker chunk
/// built from the keyframes seen on the way.
///
/// Only the keyframe timestamps are retained, so this keeps the one-GOP-at-a-time streaming.
struct WithKeyframeMarker<I> {
    sample_chunks: I,
    entity_path: EntityPath,
    timeline_name: TimelineName,
    timeline_type: TimeType,
    keyframe_times: Vec<i64>,
    done: bool,
}

impl<I> Iterator for WithKeyframeMarker<I>
where
    I: Iterator<Item = Result<(Chunk, Vec<i64>), Mp4Error>>,
{
    type Item = Result<Chunk, Mp4Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        match self.sample_chunks.next() {
            Some(Ok((chunk, keyframe_times))) => {
                self.keyframe_times.extend(keyframe_times);
                Some(Ok(chunk))
            }
            Some(Err(err)) => {
                // A failed stream is over. Without this the marker chunk would
                // still be emitted once the source runs dry, describing only the
                // GOPs that happened to make it out before the failure.
                self.done = true;
                Some(Err(err))
            }
            None => {
                self.done = true;
                build_keyframe_chunk(
                    &self.entity_path,
                    self.timeline_name,
                    self.timeline_type,
                    std::mem::take(&mut self.keyframe_times),
                )
                .transpose()
            }
        }
    }
}

/// A window cut resolved against one segment: emit the samples of
/// `[emit_from, end)`, timestamped `pts − start` (window-relative).
///
/// `emit_from` never precedes `start`, so emitted timestamps are never negative:
/// the output is frame-exact.
#[derive(Clone, Copy)]
struct WindowCut {
    emit_from_ns: i64,
    start_ns: i64,
    end_ns: i64,
}

impl WindowCut {
    /// A degenerate cut that only guards an end boundary: no rebase — used to trim
    /// ffmpeg's already-rebased head output at the splice seam.
    fn trim_to(end_ns: i64) -> Self {
        Self {
            emit_from_ns: 0,
            start_ns: 0,
            end_ns,
        }
    }

    fn keeps(&self, pts_ns: i64) -> bool {
        (self.emit_from_ns..self.end_ns).contains(&pts_ns)
    }
}

/// How a window is served when the source itself needs no re-encode.
enum WindowRoute {
    /// A GOP aligned window cut, copies the source video's GOPs directly.
    Direct(WindowCut),

    /// Frame-exact with a start that splits a GOP: `head` = [start, next keyframe)
    /// is re-encoded (paying the split GOP's decode at conversion time, so the
    /// first output frame becomes a keyframe), and [next keyframe, end) is served
    /// directly, unchanged.
    ///
    /// `head_cut` trims the re-encoded output to [0, next keyframe − start) so
    /// ffmpeg's `-to` boundary can never leak a frame the tail also emits.
    /// `tail_cut` is `None` when the window ends inside the split GOP — the head
    /// is then the whole window.
    Split {
        head: TimeWindow,
        head_cut: WindowCut,
        tail_cut: Option<WindowCut>,
    },
}

impl WindowRoute {
    fn resolve(desc: &VideoDataDescription, window: TimeWindow) -> Result<Self, Mp4Error> {
        let timescale = desc.timescale.ok_or(Mp4Error::NoTimescale)?;
        let as_ns =
            |duration: std::time::Duration| i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX);
        let start_ns = as_ns(window.start());
        let end_ns = as_ns(window.end());

        // The single covering-keyframe lookup — the cut, the aligned check, and the
        // next keyframe below all derive from it, so they can never disagree.
        let start = Time::from_secs(window.start().as_secs_f64(), timescale);
        let covering = desc.presentation_time_keyframe_index(start);
        let keyframe_pts_ns = |index: usize| -> Option<i64> {
            desc.keyframe_indices
                .get(index)
                .and_then(|&sample| desc.samples[sample].sample())
                .map(|sample| sample.presentation_timestamp.into_nanos(timescale))
        };

        // The pts of the keyframe covering `start` — the latest keyframe at or
        // before it. The window is *aligned* (served directly) when that keyframe
        // is not strictly before `start`: exact equality, a start that precedes the
        // first keyframe (`None` — no frame exists before it either), and the
        // lookup's nearest-time-unit rounding resolving to a keyframe just *after*
        // `start` all collapse onto `emit_from == start` via the `min`.
        let emit_from_ns = covering
            .and_then(keyframe_pts_ns)
            .map_or(start_ns, |keyframe_ns| keyframe_ns.min(start_ns));
        let cut = WindowCut {
            emit_from_ns,
            start_ns,
            end_ns,
        };

        if cut.emit_from_ns == cut.start_ns {
            return Ok(Self::Direct(cut));
        }

        // The first keyframe strictly after `start`: one past the covering one.
        let next_keyframe_ns = keyframe_pts_ns(covering.map_or(0, |index| index + 1));
        let (head_end_ns, tail_cut) = match next_keyframe_ns {
            Some(keyframe_ns) if keyframe_ns < end_ns => (
                keyframe_ns,
                Some(WindowCut {
                    emit_from_ns: keyframe_ns,
                    start_ns,
                    end_ns,
                }),
            ),
            // The window ends inside the split GOP (or the split GOP is the last
            // one): the head covers the whole window.
            _ => (end_ns, None),
        };

        let head = TimeWindow::new(
            window.start(),
            std::time::Duration::from_nanos(u64::try_from(head_end_ns).unwrap_or(u64::MAX)),
        )
        .ok_or_else(|| {
            // Unreachable: both candidates for `head_end_ns` exceed the start (the
            // next keyframe is strictly after it; the window end always is).
            Mp4Error::SampleConversion("window head must be non-empty".to_owned())
        })?;

        Ok(Self::Split {
            head,
            head_cut: WindowCut::trim_to(head_end_ns - start_ns),
            tail_cut,
        })
    }
}

/// A demuxed, validated mp4 segment — everything needed to emit its GOP chunks.
///
/// The direct path produces exactly one (the whole source); the transcode path
/// produces one per GOP fragment.
struct Segment {
    /// Read sample bytes from here on demand, by seeking to each sample's span.
    reader: Box<dyn ReadSeek>,
    desc: VideoDataDescription,
    timescale: re_video::Timescale,
}

impl Segment {
    fn new(reader: Box<dyn ReadSeek>, desc: VideoDataDescription) -> Result<Self, Mp4Error> {
        let timescale = desc.timescale.ok_or(Mp4Error::NoTimescale)?;

        // `Mode::Stream` requires each segment to begin on a keyframe: a decoder
        // cannot start mid-GOP. (For the direct path this is the whole source; for
        // the transcode path it's every fragment, guaranteed by ffmpeg's
        // `frag_keyframe`.) Samples before the first keyframe would otherwise be
        // silently dropped by `sample_ranges`, whose GOP ranges start at the first
        // keyframe.
        if !desc.samples.is_empty()
            && desc.keyframe_indices.first() != Some(&desc.samples.min_index())
        {
            return Err(Mp4Error::SamplesBeforeFirstKeyframe);
        }

        Ok(Self {
            reader,
            desc,
            timescale,
        })
    }
}

/// Turn one segment into its per-GOP (or per-sample) chunks, reading sample bytes
/// on demand. A segment that failed to demux yields a single `Err`.
///
/// Each chunk comes with the times of the keyframes in it, for
/// [`WithKeyframeMarker`] to collect.
fn gop_chunks(
    segment: Result<(Segment, Option<WindowCut>), Mp4Error>,
    emission: Emission,
) -> impl Iterator<Item = Result<(Chunk, Vec<i64>), Mp4Error>> {
    let (segment, window_cut) = match segment {
        Ok(pair) => pair,
        Err(err) => return Either::Left(std::iter::once(Err(err))),
    };
    let Segment {
        mut reader,
        desc,
        timescale,
    } = segment;

    let ranges = sample_ranges(&desc, timescale, &emission, window_cut);
    Either::Right(ranges.into_iter().filter_map(move |range| {
        // `Ok(None)` (a GOP whose samples were all unloaded or cut away) → skip via
        // `transpose`.
        build_gop_chunk(&mut *reader, &desc, timescale, &emission, range, window_cut).transpose()
    }))
}

/// Spawn the `-bf 0` transcode and yield one demuxed [`Segment`] per GOP fragment
/// ffmpeg streams back.
#[cfg(not(target_arch = "wasm32"))]
fn transcoded_segments(
    input: StreamInput,
    source_codec: re_video::VideoCodec,
    transcode: &Mp4TranscodeOptions,
    time_window: Option<TimeWindow>,
    debug_name: &str,
) -> Result<impl Iterator<Item = Result<Segment, Mp4Error>> + use<>, Mp4Error> {
    // ffmpeg needs a seekable file (an mp4's `moov` can trail its `mdat`, so a
    // pipe can't be demuxed). Only the file-path input can offer that.
    let StreamInput::Path(path) = input else {
        return Err(Mp4Error::TranscodeRequiresSeekableFile);
    };

    let chunks = re_video::transcode_mp4(&path, source_codec, transcode, time_window, debug_name)
        .map_err(|err| map_ffmpeg_err(&err))?;
    let scanner = FragmentScanner::new(ChunkReader::new(chunks), debug_name)?;

    let debug_name = debug_name.to_owned();
    Ok(scanner.map(move |mini_mp4| {
        // Each item is a complete `init + fragment` mini-mp4 the ordinary demuxer
        // handles, so sample/timestamp/keyframe/Annex-B logic is all reused.
        let mini_mp4 = mini_mp4?;
        let size = mini_mp4.len() as u64;
        let mut reader = std::io::Cursor::new(mini_mp4);
        let desc = VideoDataDescription::load_mp4_from_reader(&mut reader, size, &debug_name)?;
        Segment::new(Box::new(reader), desc)
    }))
}

/// Map a `re_video` `FFmpeg` error into an [`Mp4Error`], preserving the original
/// message and appending a download hint when the executable is missing.
#[cfg(not(target_arch = "wasm32"))]
fn map_ffmpeg_err(err: &re_video::FFmpegError) -> Mp4Error {
    // A missing encoder for the requested output codec gets its own actionable
    // error rather than being flattened into the generic transcode message.
    if let re_video::FFmpegError::NoEncoderForCodec { codec } = err {
        return Mp4Error::NoEncoderAvailable {
            codec: codec.clone(),
        };
    }

    let mut msg = err.to_string();
    if matches!(err, re_video::FFmpegError::FFmpegNotInstalled)
        && let Some(url) = re_video::ffmpeg_download_url()
    {
        msg = format!("{msg} You can download a build of FFmpeg at {url}");
    }
    Mp4Error::Transcode(msg)
}

/// Adapts the transcode's chunk iterator into a [`Read`] for [`FragmentScanner`].
///
/// `re_video` hands back the fragmented mp4 as an iterator of arbitrary byte
/// chunks (one `ffmpeg` `OutputChunk` each); [`FragmentScanner`] frames mp4
/// boxes and needs byte-level reads that span chunk boundaries. This keeps at
/// most one chunk buffered, preserving the one-GOP-at-a-time streaming.
#[cfg(not(target_arch = "wasm32"))]
struct ChunkReader<I> {
    chunks: I,

    /// The chunk currently being drained, and how far into it we've read.
    current: Vec<u8>,
    pos: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl<I> ChunkReader<I> {
    fn new(chunks: I) -> Self {
        Self {
            chunks,
            current: Vec::new(),
            pos: 0,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<I: Iterator<Item = Result<Vec<u8>, re_video::FFmpegError>>> Read for ChunkReader<I> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Pull chunks until we have bytes to hand back or the stream ends. The
        // loop also skips any (unexpected) empty chunk rather than reporting EOF.
        while self.pos >= self.current.len() {
            match self.chunks.next() {
                Some(Ok(chunk)) => {
                    self.current = chunk;
                    self.pos = 0;
                }
                // A transcode failure surfaces here; `read_box`'s `?` turns it
                // into `Mp4Error::Io`, matching the previous `Read`-based design.
                Some(Err(err)) => return Err(std::io::Error::other(err.to_string())),
                None => return Ok(0),
            }
        }

        let n = (self.current.len() - self.pos).min(buf.len());
        buf[..n].copy_from_slice(&self.current[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// One mp4 box: its 4-byte type and its full bytes (header + body).
#[cfg(not(target_arch = "wasm32"))]
type Mp4Box = ([u8; 4], Vec<u8>);

/// Splits ffmpeg's fragmented-mp4 stdout into one complete `init + fragment`
/// mini-mp4 per GOP, without buffering the whole stream.
///
/// The init segment (`ftyp` + `empty_moov`) is read up front and prepended to
/// each subsequent `moof`/`mdat` fragment. A trailing `mfra` (or EOF) ends the
/// iteration.
#[cfg(not(target_arch = "wasm32"))]
struct FragmentScanner<R> {
    reader: R,
    init: Vec<u8>,

    /// A `moof` that has been read but whose `mdat` hasn't been paired yet.
    pending_moof: Option<Vec<u8>>,
    done: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl<R: Read> FragmentScanner<R> {
    fn new(mut reader: R, debug_name: &str) -> Result<Self, Mp4Error> {
        let mut init = Vec::new();
        let mut pending_moof = None;
        // Read boxes until the first `moof` — everything before it (ftyp, moov, …)
        // is the init segment.
        loop {
            match read_box(&mut reader)? {
                None => break, // no fragments at all
                Some((box_type, bytes)) => {
                    if &box_type == b"moof" {
                        pending_moof = Some(bytes);
                        break;
                    }
                    init.extend_from_slice(&bytes);
                }
            }
        }
        if init.is_empty() {
            return Err(Mp4Error::Transcode(format!(
                "ffmpeg produced no init segment for {debug_name}"
            )));
        }
        Ok(Self {
            reader,
            init,
            pending_moof,
            done: false,
        })
    }

    /// Read the next `moof` + `mdat` fragment (without the init segment).
    fn next_fragment(&mut self) -> Result<Option<Vec<u8>>, Mp4Error> {
        if self.done {
            return Ok(None);
        }
        let Some(mut fragment) = self.pending_moof.take() else {
            self.done = true;
            return Ok(None);
        };

        // Each `moof` is followed by its `mdat`.
        match read_box(&mut self.reader)? {
            Some((box_type, mdat)) if &box_type == b"mdat" => fragment.extend_from_slice(&mdat),
            _ => {
                self.done = true;
                return Err(Mp4Error::Transcode(
                    "ffmpeg fragmented mp4 has a `moof` without a following `mdat`".to_owned(),
                ));
            }
        }

        // Peek the next box: another `moof` starts the next fragment; anything
        // else (`mfra`) or EOF ends the stream.
        match read_box(&mut self.reader)? {
            Some((box_type, bytes)) if &box_type == b"moof" => self.pending_moof = Some(bytes),
            _ => self.done = true,
        }

        Ok(Some(fragment))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<R: Read> Iterator for FragmentScanner<R> {
    /// A complete, self-contained mini-mp4: the shared init segment + one GOP.
    type Item = Result<Vec<u8>, Mp4Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_fragment() {
            Ok(Some(fragment)) => {
                let mut mini_mp4 = Vec::with_capacity(self.init.len() + fragment.len());
                mini_mp4.extend_from_slice(&self.init);
                mini_mp4.extend_from_slice(&fragment);
                Some(Ok(mini_mp4))
            }
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        }
    }
}

/// Read one complete mp4 box (its 4-byte-size + 4-byte-type header plus body).
/// Returns `Ok(None)` on a clean EOF at a box boundary.
#[cfg(not(target_arch = "wasm32"))]
fn read_box<R: Read>(reader: &mut R) -> Result<Option<Mp4Box>, Mp4Error> {
    let mut header = [0u8; 8];
    if !read_exact_or_eof(reader, &mut header)? {
        return Ok(None);
    }

    let size32 = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let mut box_type = [0u8; 4];
    box_type.copy_from_slice(&header[4..8]);

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&header);

    let total = if size32 == 1 {
        // 64-bit `largesize` follows the header.
        let mut ext = [0u8; 8];
        reader.read_exact(&mut ext)?;
        bytes.extend_from_slice(&ext);
        u64::from_be_bytes(ext) as usize
    } else if size32 == 0 {
        // Box extends to EOF — read whatever remains.
        reader.read_to_end(&mut bytes)?;
        return Ok(Some((box_type, bytes)));
    } else {
        size32 as usize
    };

    if total < bytes.len() {
        return Err(Mp4Error::Transcode(format!(
            "ffmpeg produced an mp4 box with an invalid size {total}"
        )));
    }
    let already = bytes.len();
    bytes.resize(total, 0);
    reader.read_exact(&mut bytes[already..])?;
    Ok(Some((box_type, bytes)))
}

/// Fill `buf` completely; `Ok(false)` if EOF is hit before any byte is read
/// (a clean box boundary), `Err` if EOF is hit part-way through.
#[cfg(not(target_arch = "wasm32"))]
fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<bool, Mp4Error> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..])? {
            0 => {
                if filled == 0 {
                    return Ok(false);
                }
                return Err(Mp4Error::Transcode(
                    "ffmpeg output ended in the middle of an mp4 box".to_owned(),
                ));
            }
            n => filled += n,
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------------
// Shared chunk construction.
// ---------------------------------------------------------------------------

/// GOP ranges (or per-sample singleton ranges) to emit one chunk each, restricted to
/// the GOPs (or samples) that `window_cut` keeps.
fn sample_ranges(
    desc: &VideoDataDescription,
    timescale: re_video::Timescale,
    emission: &Emission,
    window_cut: Option<WindowCut>,
) -> Vec<Span<SampleIndex>> {
    let keeps = |range: &Span<SampleIndex>| {
        let Some(cut) = window_cut else {
            return true;
        };
        desc.samples[range.start]
            .sample()
            .is_some_and(|sample| cut.keeps(sample.presentation_timestamp.into_nanos(timescale)))
    };

    if emission.chunk_by_gop {
        // A GOP is keyframe-anchored, so testing its first sample against the cut
        // keeps exactly the GOPs from the cut's emit-from point up to the window
        // end; the last GOP's out-of-window samples are dropped per sample below.
        (0..desc.keyframe_indices.len())
            .filter_map(|i| desc.gop_sample_range_for_keyframe(i))
            .filter(keeps)
            .collect()
    } else {
        desc.samples
            .iter_indexed()
            .filter_map(|(idx, sample)| match sample {
                SampleMetadataState::Present(_) => Some(Span::from_start_len(idx, 1)),
                SampleMetadataState::Unloaded { .. } => {
                    re_log::warn_once!(
                        "Skipping unloaded sample {idx} in mp4 demux (entity path: {})",
                        emission.entity_path
                    );
                    None
                }
            })
            .filter(keeps)
            .collect()
    }
}

fn build_codec_chunk(entity_path: &EntityPath, codec: VideoCodec) -> Result<Chunk, Mp4Error> {
    let chunk = Chunk::builder(entity_path.clone())
        .with_archetype(
            RowId::new(),
            TimePoint::default(),
            &VideoStream::update_fields().with_codec(codec),
        )
        .build()?;
    Ok(chunk)
}

/// Build a single chunk for `range`, reading sample bytes from `reader` on
/// demand, or `Ok(None)` if every sample in the range was unloaded.
///
/// Returns the chunk plus the times of the keyframes in it.
fn build_gop_chunk(
    reader: &mut dyn ReadSeek,
    desc: &VideoDataDescription,
    timescale: re_video::Timescale,
    emission: &Emission,
    range: Span<SampleIndex>,
    window_cut: Option<WindowCut>,
) -> Result<Option<(Chunk, Vec<i64>)>, Mp4Error> {
    let mut time_values: Vec<i64> = Vec::with_capacity(range.len);
    let mut sample_blobs: Vec<Vec<u8>> = Vec::with_capacity(range.len);
    let mut keyframe_times: Vec<i64> = Vec::new();

    let mut sample_bytes = vec![];
    for sample_idx in range {
        let SampleMetadataState::Present(meta) = &desc.samples[sample_idx] else {
            re_log::warn_once!(
                "Skipping unloaded sample {sample_idx} in mp4 demux (entity path: {})",
                emission.entity_path
            );
            continue;
        };

        let mut pts_ns = meta.presentation_timestamp.into_nanos(timescale);
        if let Some(cut) = window_cut {
            if !cut.keeps(pts_ns) {
                continue; // the tail GOP's samples past the window end
            }
            // Window-relative. Never negative: emission never precedes the window
            // start (see [`WindowCut`]).
            pts_ns -= cut.start_ns;
        }
        time_values.push(pts_ns);

        // mp4 demux only emits `VideoSource::Span` (`VideoSource::Id` is never
        // produced by `re_video::demux::mp4`).
        let VideoSource::Span(span) = meta.source else {
            return Err(Mp4Error::SampleConversion(format!(
                "sample {sample_idx} has a non-span source; mp4 demux only produces spans"
            )));
        };

        struct FullSource<'a>(&'a [u8]);

        impl GetVideoSource for FullSource<'_> {
            fn get_video_chunk(&self, _source: VideoSource) -> &[u8] {
                self.0
            }

            fn require_video_source(&self, _source: VideoSource) {}

            fn indicate_video_source(&self, _source: VideoSource) {}
        }

        let sample_len = usize::try_from(span.len).map_err(|_err| {
            Mp4Error::SampleConversion(format!("sample {sample_idx} is too large to read"))
        })?;
        reader.seek(SeekFrom::Start(span.start))?;
        sample_bytes.resize(sample_len, 0);
        reader.read_exact(&mut sample_bytes)?;

        let chunk = meta
            .get(&FullSource(sample_bytes.as_slice()), sample_idx)
            .ok_or_else(|| {
                Mp4Error::SampleConversion(format!(
                    "sample {sample_idx} could not be read from the mp4 buffer"
                ))
            })?;

        sample_blobs.push(
            desc.sample_data_in_stream_format(&chunk)
                .map_err(|err| Mp4Error::SampleConversion(err.to_string()))?,
        );
        if meta.is_sync {
            keyframe_times.push(pts_ns);
        }
    }

    if time_values.is_empty() {
        // Every sample in this range was unloaded; there is nothing to emit.
        return Ok(None);
    }

    let timeline = Timeline::new(emission.timeline_name, emission.timeline_type);
    // Stream mode always emits B-frame-free samples (either the source had none,
    // or they were transcoded away), so the samples are in PTS order.
    let time_column = TimeColumn::new(
        Some(true),
        timeline,
        arrow::buffer::ScalarBuffer::from(time_values),
    );

    let components: Vec<_> = VideoStream::update_fields()
        .with_many_sample(sample_blobs)
        .columns_of_unit_batches()
        .map_err(|err| {
            Mp4Error::SampleConversion(format!("Failed to construct sample chunk: {err}"))
        })?
        .collect();

    let chunk = Chunk::from_auto_row_ids(
        ChunkId::new(),
        emission.entity_path.clone(),
        std::iter::once((*timeline.name(), time_column)).collect(),
        components.into_iter().collect(),
    )?;

    Ok(Some((chunk, keyframe_times)))
}

/// Build the sparse `IsKeyframe` marker chunk: one row per keyframe, all `true`.
///
/// `Ok(None)` if the stream had no keyframe at all.
fn build_keyframe_chunk(
    entity_path: &EntityPath,
    timeline_name: TimelineName,
    timeline_type: TimeType,
    keyframe_times: Vec<i64>,
) -> Result<Option<Chunk>, Mp4Error> {
    if keyframe_times.is_empty() {
        return Ok(None);
    }

    let num_keyframes = keyframe_times.len();
    let timeline = Timeline::new(timeline_name, timeline_type);
    let time_column = TimeColumn::new(
        None,
        timeline,
        arrow::buffer::ScalarBuffer::from(keyframe_times),
    );

    let components: Vec<_> = VideoStream::update_fields()
        .with_many_is_keyframe(std::iter::repeat_n(true, num_keyframes))
        .columns_of_unit_batches()
        .map_err(|err| {
            Mp4Error::SampleConversion(format!("Failed to construct is_keyframe chunk: {err}"))
        })?
        .collect();

    let chunk = Chunk::from_auto_row_ids(
        ChunkId::new(),
        entity_path.clone(),
        std::iter::once((*timeline.name(), time_column)).collect(),
        components.into_iter().collect(),
    )?;

    Ok(Some(chunk))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use re_chunk::EntityPath;
    use re_log_types::{TimeType, TimelineName};
    use re_video::VideoDataDescription;
    use std::assert_matches;

    use super::{FragmentScanner, WithKeyframeMarker};
    use crate::Mp4Error;

    /// An error ends the stream. The keyframe marker is built from what the
    /// samples reported on the way past, so emitting it after a failure would
    /// describe only the GOPs that made it out before the error — a marker that
    /// looks complete but silently covers part of the video.
    #[test]
    fn the_keyframe_marker_is_dropped_when_the_stream_fails() {
        let mut iter = WithKeyframeMarker {
            sample_chunks: std::iter::once(Err(Mp4Error::SampleConversion("boom".to_owned()))),
            entity_path: EntityPath::from("video"),
            timeline_name: TimelineName::from_static_str("video"),
            timeline_type: TimeType::DurationNs,
            // Two GOPs got through before the failure.
            keyframe_times: vec![0, 1000],
            done: false,
        };

        assert_matches!(iter.next(), Some(Err(_)));
        assert!(iter.next().is_none(), "no marker chunk after an error");
    }

    /// Build a minimal mp4 box: 4-byte big-endian size, 4-byte type, then body.
    fn mp4_box(box_type: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut bytes = ((8 + body.len()) as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(box_type);
        bytes.extend_from_slice(body);
        bytes
    }

    /// The multi-fragment case the end-to-end test can't reach (our H.264/H.265
    /// fixtures transcode to a single GOP): a `moof`/`mdat` pair per fragment,
    /// chained until a trailing `mfra`. Each yields a complete `init + fragment`
    /// mini-mp4. `FragmentScanner` only frames boxes, so the bodies are arbitrary.
    #[test]
    fn yields_one_init_plus_fragment_mini_mp4_per_gop() {
        let ftyp = mp4_box(b"ftyp", b"isomiso2");
        let moov = mp4_box(b"moov", b"fake-init-metadata");
        let init: Vec<u8> = [ftyp, moov].concat();

        let fragments: Vec<Vec<u8>> = (0..3u8)
            .map(|i| [mp4_box(b"moof", &[i; 4]), mp4_box(b"mdat", &[0xAB; 16])].concat())
            .collect();

        let mut stream = init.clone();
        for fragment in &fragments {
            stream.extend_from_slice(fragment);
        }
        stream.extend_from_slice(&mp4_box(b"mfra", b"index"));

        let scanner = FragmentScanner::new(std::io::Cursor::new(stream), "test").unwrap();
        let got: Vec<Vec<u8>> = scanner.map(Result::unwrap).collect();

        let expected: Vec<Vec<u8>> = fragments
            .iter()
            .map(|fragment| [init.clone(), fragment.clone()].concat())
            .collect();
        assert_eq!(
            got, expected,
            "one init+fragment mini-mp4 per moof+mdat pair"
        );
    }

    /// An init segment with no fragments (`ftyp` + `moov`, no `moof`) yields nothing.
    #[test]
    fn init_only_stream_yields_no_fragments() {
        let stream = [mp4_box(b"ftyp", b"isom"), mp4_box(b"moov", b"meta")].concat();
        let mut scanner = FragmentScanner::new(std::io::Cursor::new(stream), "test").unwrap();
        assert!(scanner.next().is_none());
    }

    /// A B-frame-free description at nanosecond timescale: one `Present` sample per
    /// pts, keyframes at the given pts (each must also be a sample pts).
    fn desc_with_samples(keyframe_pts_ns: &[i64], sample_pts_ns: &[i64]) -> VideoDataDescription {
        use re_video::{
            SampleMetadata, SamplesStatistics, Span, StableIndexDeque, Time, Timescale,
            VideoDeliveryMethod, VideoSource,
        };

        let samples: StableIndexDeque<_> = sample_pts_ns
            .iter()
            .enumerate()
            .map(|(frame_nr, &pts)| {
                super::SampleMetadataState::Present(SampleMetadata {
                    is_sync: keyframe_pts_ns.contains(&pts),
                    frame_nr: frame_nr as u32,
                    decode_timestamp: Time::new(pts),
                    presentation_timestamp: Time::new(pts),
                    duration: None,
                    source: VideoSource::Span(Span { start: 0, len: 0 }),
                })
            })
            .collect();
        let keyframe_indices = keyframe_pts_ns
            .iter()
            .map(|kf| {
                sample_pts_ns
                    .iter()
                    .position(|pts| pts == kf)
                    .expect("keyframe pts must be a sample pts")
            })
            .collect();

        VideoDataDescription {
            codec: re_video::VideoCodec::AV1,
            encoding_details: None,
            timescale: Some(Timescale::NANOSECOND),
            delivery_method: VideoDeliveryMethod::Static {
                duration: Time::new(*sample_pts_ns.last().expect("at least one sample")),
            },
            keyframe_indices,
            samples,
            samples_statistics: SamplesStatistics::NO_BFRAMES,
            mp4_tracks: Default::default(),
        }
    }

    fn resolve(desc: &VideoDataDescription, window: super::TimeWindow) -> super::WindowRoute {
        super::WindowRoute::resolve(desc, window).expect("timescale present")
    }

    /// A window starting before the first keyframe resolves to the direct cut:
    /// no frame exists before the start, so the direct slice is already
    /// frame-exact. (No mp4 fixture starts late, so this branch is only
    /// reachable synthetically.)
    #[test]
    fn window_before_the_first_keyframe_resolves_direct() {
        let ms = |v: u64| std::time::Duration::from_millis(v);
        let desc = desc_with_samples(
            &[500_000_000],
            &[500_000_000, 600_000_000, 700_000_000, 800_000_000],
        );
        let window = super::TimeWindow::new(ms(100), ms(800)).expect("valid window");

        let super::WindowRoute::Direct(cut) = resolve(&desc, window) else {
            panic!("a window preceding the first keyframe is served directly");
        };

        assert_eq!(
            cut.emit_from_ns, 100_000_000,
            "no covering keyframe → emission begins at the window start itself"
        );
        assert!(cut.keeps(500_000_000));
        assert!(!cut.keeps(99_999_999));
        assert!(!cut.keeps(800_000_000), "the end stays exclusive");
    }

    /// A window whose start lands on a keyframe resolves to the direct cut —
    /// already frame-exact there, so no re-encode is planned.
    #[test]
    fn window_with_aligned_start_resolves_direct() {
        let ms = |v: u64| std::time::Duration::from_millis(v);
        let desc = desc_with_samples(
            &[0, 500_000_000],
            &[0, 250_000_000, 500_000_000, 750_000_000, 900_000_000],
        );
        let window = super::TimeWindow::new(ms(500), ms(900)).expect("valid window");

        let super::WindowRoute::Direct(cut) = resolve(&desc, window) else {
            panic!("a window starting on a keyframe needs no re-encode");
        };
        assert_eq!(cut.emit_from_ns, 500_000_000);
        assert!(
            !cut.keeps(250_000_000),
            "nothing before the start is emitted"
        );
    }

    /// A window whose start splits a GOP resolves to a split: the head re-encodes
    /// [start, next keyframe), the tail serves the rest directly.
    #[test]
    fn window_with_mid_gop_start_resolves_split() {
        let ms = |v: u64| std::time::Duration::from_millis(v);
        let desc = desc_with_samples(
            &[0, 500_000_000],
            &[0, 250_000_000, 500_000_000, 750_000_000, 900_000_000],
        );
        let window = super::TimeWindow::new(ms(200), ms(900)).expect("valid window");

        let super::WindowRoute::Split {
            head,
            head_cut,
            tail_cut,
        } = resolve(&desc, window)
        else {
            panic!("a mid-GOP window must split");
        };

        assert_eq!(head.start(), ms(200));
        assert_eq!(head.end(), ms(500), "the head ends at the next keyframe");
        assert!(head_cut.keeps(0) && !head_cut.keeps(300_000_000));
        let tail = tail_cut.expect("the window extends past the split GOP");
        assert_eq!(tail.emit_from_ns, 500_000_000);
        assert!(
            !tail.keeps(250_000_000),
            "the tail starts at the next keyframe — the head owns everything before"
        );
        assert!(!tail.keeps(900_000_000), "the end stays exclusive");
    }

    /// A window that ends inside the GOP its start splits has no tail: the
    /// re-encoded head is the whole window.
    #[test]
    fn window_inside_one_gop_resolves_headless_split() {
        let ms = |v: u64| std::time::Duration::from_millis(v);
        let desc = desc_with_samples(
            &[0, 500_000_000],
            &[0, 250_000_000, 500_000_000, 750_000_000, 900_000_000],
        );
        let window = super::TimeWindow::new(ms(200), ms(400)).expect("valid window");

        let super::WindowRoute::Split {
            head,
            head_cut,
            tail_cut,
        } = resolve(&desc, window)
        else {
            panic!("a mid-GOP window must split");
        };

        assert_eq!(head.end(), ms(400), "the head covers the whole window");
        assert!(!head_cut.keeps(200_000_000), "trimmed at end − start");
        assert!(
            tail_cut.is_none(),
            "the window never reaches the next keyframe"
        );
    }
}
