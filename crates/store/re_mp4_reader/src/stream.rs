//! Stream-mode chunk emission: demux the mp4 with `re_video` and emit
//! `VideoStream` chunks (one static codec chunk plus per-GOP or per-sample
//! `VideoSample` / `IsKeyframe` chunks).
//!
//! Both modes reduce to the same pipeline — *emit the codec chunk, then turn a
//! sequence of demuxed [`Segment`]s into GOP chunks*:
//! - no B-frames: one segment, the source itself, emitted directly;
//! - B-frames: `VideoStream` can't model DTS != PTS, so ffmpeg re-encodes and
//!   streams back a fragmented mp4, and each `moof` fragment becomes one segment.
//!   Only one GOP is resident at a time.

use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::Path;

use itertools::Either;

use re_chunk::{Chunk, ChunkId, EntityPath, RowId, TimeColumn, TimePoint};
use re_log_types::{TimeType, Timeline, TimelineName};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::VideoCodec;
use re_video::player::GetVideoSource;
use re_video::{SampleIndex, SampleMetadataState, VideoDataDescription, VideoSource};

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

/// Build a chunk iterator for stream mode.
///
/// Demuxes the source once to inspect it, then emits the static codec chunk
/// followed by the GOP chunks of each [`Segment`]. The image-sequence-codec,
/// keyframe, and timescale checks are performed eagerly (here and in
/// [`Segment::new`]) so callers see those errors from this constructor rather
/// than from the first `.next()` on the iterator.
pub(crate) fn iter_chunks(
    input: StreamInput,
    entity_path: &EntityPath,
    timeline_name: TimelineName,
    chunk_by_gop: bool,
    timeline_type: TimeType,
    ffmpeg_override: Option<&Path>,
    debug_name: &str,
) -> Result<ChunkIter, Mp4Error> {
    re_tracing::profile_function!();

    let mut reader = input.open()?;
    let size = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;
    let desc = VideoDataDescription::load_mp4_from_reader(&mut reader, size, debug_name)?;

    // ffmpeg keeps the source codec, so this is the emitted `VideoStream` codec
    // whether or not we transcode.
    let mapped_codec = VideoCodec::try_from(desc.codec.clone())
        .map_err(|_err| Mp4Error::ImageSequenceInStreamMode)?;

    let segments: Box<dyn Iterator<Item = Result<Segment, Mp4Error>>> =
        if desc.samples_statistics.dts_always_equal_pts {
            // No container-level reordering. Covers both B-frame-free H.26x and codecs
            // that reorder *in-band* (AV1 `show_existing_frame`, VP9 superframes), which
            // keep DTS == PTS at the container. One segment, read directly from the source
            // (sample bytes are fetched on demand, so the whole file is never resident).
            Box::new(std::iter::once(Segment::new(reader, desc)))
        } else {
            // Container-level B-frame reordering (DTS != PTS). `VideoStream` can't model
            // this yet (#10090). In practice only H.264/H.265 express reordering this way,
            // and only those can be re-encoded with `-bf 0`, so reject any other codec here
            // with a clear message rather than letting the encoder mapping fail deep inside
            // the ffmpeg call.
            if !matches!(mapped_codec, VideoCodec::H264 | VideoCodec::H265) {
                return Err(Mp4Error::BFramesUnsupportedCodec {
                    codec: mapped_codec,
                });
            }

            // ffmpeg re-encodes and streams back a fragmented mp4, one
            // segment per GOP fragment.
            drop(reader);
            #[cfg(not(target_arch = "wasm32"))]
            {
                Box::new(transcoded_segments(
                    input,
                    desc.codec.clone(),
                    ffmpeg_override,
                    debug_name,
                )?)
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _ = (input, ffmpeg_override);
                return Err(Mp4Error::BFramesRequireFfmpeg);
            }
        };

    let entity_path = entity_path.clone();
    let codec_chunk = build_codec_chunk(&entity_path, mapped_codec);
    let gop_chunks = segments.flat_map(move |segment| {
        gop_chunks(
            segment,
            entity_path.clone(),
            timeline_name,
            timeline_type,
            chunk_by_gop,
        )
    });
    Ok(Box::new(std::iter::chain(
        std::iter::once(codec_chunk),
        gop_chunks,
    )))
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
fn gop_chunks(
    segment: Result<Segment, Mp4Error>,
    entity_path: EntityPath,
    timeline_name: TimelineName,
    timeline_type: TimeType,
    chunk_by_gop: bool,
) -> impl Iterator<Item = Result<Chunk, Mp4Error>> {
    let Segment {
        mut reader,
        desc,
        timescale,
    } = match segment {
        Ok(segment) => segment,
        Err(err) => return Either::Left(std::iter::once(Err(err))),
    };

    let ranges = sample_ranges(&desc, chunk_by_gop, &entity_path);
    Either::Right(ranges.into_iter().filter_map(move |range| {
        // `Ok(None)` (a GOP whose samples were all unloaded) → skip via `transpose`.
        build_gop_chunk(
            &mut *reader,
            &desc,
            timescale,
            timeline_name,
            timeline_type,
            &entity_path,
            range,
        )
        .transpose()
    }))
}

/// Spawn the `-bf 0` transcode and yield one demuxed [`Segment`] per GOP fragment
/// ffmpeg streams back.
#[cfg(not(target_arch = "wasm32"))]
fn transcoded_segments(
    input: StreamInput,
    source_codec: re_video::VideoCodec,
    ffmpeg_override: Option<&Path>,
    debug_name: &str,
) -> Result<impl Iterator<Item = Result<Segment, Mp4Error>> + use<>, Mp4Error> {
    // ffmpeg needs a seekable file (an mp4's `moov` can trail its `mdat`, so a
    // pipe can't be demuxed). Only the file-path input can offer that.
    let StreamInput::Path(path) = input else {
        return Err(Mp4Error::BFramesFromInMemoryBytes);
    };

    let chunks =
        re_video::transcode_mp4_drop_b_frames(&path, source_codec, ffmpeg_override, debug_name)
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

/// GOP ranges (or per-sample singleton ranges) to emit one chunk each.
fn sample_ranges(
    desc: &VideoDataDescription,
    chunk_by_gop: bool,
    entity_path: &EntityPath,
) -> Vec<Range<SampleIndex>> {
    if chunk_by_gop {
        (0..desc.keyframe_indices.len())
            .filter_map(|i| desc.gop_sample_range_for_keyframe(i))
            .collect()
    } else {
        desc.samples
            .iter_indexed()
            .filter_map(|(idx, sample)| match sample {
                SampleMetadataState::Present(_) => Some(idx..idx + 1),
                SampleMetadataState::Unloaded { .. } => {
                    re_log::warn_once!(
                        "Skipping unloaded sample {idx} in mp4 demux (entity path: {entity_path})"
                    );
                    None
                }
            })
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
fn build_gop_chunk(
    reader: &mut dyn ReadSeek,
    desc: &VideoDataDescription,
    timescale: re_video::Timescale,
    timeline_name: TimelineName,
    timeline_type: TimeType,
    entity_path: &EntityPath,
    range: Range<SampleIndex>,
) -> Result<Option<Chunk>, Mp4Error> {
    let mut time_values: Vec<i64> = Vec::with_capacity(range.len());
    let mut sample_blobs: Vec<Vec<u8>> = Vec::with_capacity(range.len());
    let mut is_keyframe: Vec<bool> = Vec::with_capacity(range.len());

    let mut sample_bytes = vec![];
    for sample_idx in range {
        let SampleMetadataState::Present(meta) = &desc.samples[sample_idx] else {
            re_log::warn_once!(
                "Skipping unloaded sample {sample_idx} in mp4 demux (entity path: {entity_path})"
            );
            continue;
        };

        let pts_ns = meta.presentation_timestamp.into_nanos(timescale);
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

        let byte_range = span.range_usize();
        reader.seek(SeekFrom::Start(byte_range.start as u64))?;
        sample_bytes.resize(byte_range.len(), 0);
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
        is_keyframe.push(meta.is_sync);
    }

    if time_values.is_empty() {
        // Every sample in this range was unloaded; there is nothing to emit.
        return Ok(None);
    }

    let timeline = Timeline::new(timeline_name, timeline_type);
    // Stream mode always emits B-frame-free samples (either the source had none,
    // or they were transcoded away), so the samples are in PTS order.
    let time_column = TimeColumn::new(
        Some(true),
        timeline,
        arrow::buffer::ScalarBuffer::from(time_values),
    );

    let components: Vec<_> = VideoStream::update_fields()
        .with_many_sample(sample_blobs)
        .with_many_is_keyframe(is_keyframe)
        .columns_of_unit_batches()
        .map_err(|err| Mp4Error::SampleConversion(err.to_string()))?
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
    use super::FragmentScanner;

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
}
