//! Stream-mode chunk emission: demux the mp4 with `re_video` and emit
//! `VideoStream` chunks (one static codec chunk plus per-GOP or per-sample
//! `VideoSample` / `IsKeyframe` chunks).

use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;

use re_chunk::{Chunk, ChunkId, EntityPath, RowId, TimeColumn, TimePoint};
use re_log_types::{TimeType, Timeline};
use re_sdk_types::archetypes::VideoStream;
use re_sdk_types::components::VideoCodec;
use re_video::{SampleIndex, SampleMetadataState, VideoDataDescription, VideoSource};

use crate::Mp4Error;

/// Build a chunk iterator for stream mode.
///
/// The B-frame and image-sequence-codec checks are performed eagerly so callers
/// see those errors from this constructor rather than from the first `.next()`
/// call on the iterator.
#[expect(clippy::fn_params_excessive_bools)]
pub(crate) fn iter_chunks<R: Read + Seek>(
    mut reader: R,
    size: u64,
    entity_path: &EntityPath,
    timeline_name: &str,
    chunk_by_gop: bool,
    timeline_type: TimeType,
    allow_b_frames: bool,
    debug_name: &str,
) -> Result<StreamChunkIter<R>, Mp4Error> {
    re_tracing::profile_function!();

    let desc = VideoDataDescription::load_mp4_from_reader(&mut reader, size, debug_name)?;

    // `VideoStream` archetype does not yet model differing DTS/PTS (#10090).
    // `allow_b_frames = true` lets callers opt into raw sample bytes — useful
    // when the consumer is about to transcode them downstream.
    let has_b_frames = !desc.samples_statistics.dts_always_equal_pts;
    if has_b_frames && !allow_b_frames {
        return Err(Mp4Error::BFramesInStreamMode);
    }

    let mapped_codec = VideoCodec::try_from(desc.codec.clone())
        .map_err(|_err| Mp4Error::ImageSequenceInStreamMode)?;

    let timescale = desc.timescale.ok_or(Mp4Error::NoTimescale)?;

    // `Mode::Stream` requires the byte stream to begin on a keyframe: a decoder
    // cannot start mid-GOP. Samples before the first keyframe would otherwise be
    // silently dropped (`chunk_by_gop`, since GOP ranges start at the first
    // keyframe) or emitted without a decodable reference frame (per-sample), so
    // reject them eagerly here rather than produce an undecodable stream.
    if !desc.samples.is_empty() && desc.keyframe_indices.first() != Some(&desc.samples.min_index())
    {
        return Err(Mp4Error::SamplesBeforeFirstKeyframe);
    }

    // Pre-compute GOP ranges (or per-sample singleton ranges) up front. The
    // iterator just walks this list — keeps `Iterator::next` allocation-free
    // for the bookkeeping (each chunk's payload is still allocated, of course).
    let ranges: Vec<Range<SampleIndex>> = if chunk_by_gop {
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
    };

    Ok(StreamChunkIter {
        reader,
        desc,
        timeline_name: timeline_name.to_owned(),
        timeline_type,
        has_b_frames,
        entity_path: entity_path.clone(),
        timescale,
        mapped_codec,
        ranges,
        cursor: 0,
        static_codec_emitted: false,
    })
}

/// Iterator over the chunks of a stream-mode mp4.
///
/// The first item is the static `VideoStream(codec=…)` chunk. Subsequent items
/// are per-GOP (or per-sample) `VideoSample` / `IsKeyframe` chunks.
pub(crate) struct StreamChunkIter<R> {
    /// The `Read + Seek` source the mp4 was demuxed from. Each sample's bytes are
    /// read on demand by seeking to its byte span, so the whole file never has to
    /// be resident in memory.
    reader: R,
    desc: VideoDataDescription,
    timeline_name: String,
    timeline_type: TimeType,

    /// True when the source mp4 has B-frames and the caller opted in via
    /// `allow_b_frames = true`. Forces the time column to be marked unsorted
    /// because PTS in decode order is not monotonic in this case.
    has_b_frames: bool,
    entity_path: EntityPath,
    timescale: re_video::Timescale,
    mapped_codec: VideoCodec,
    ranges: Vec<Range<SampleIndex>>,
    cursor: usize,
    static_codec_emitted: bool,
}

impl<R: Read + Seek> Iterator for StreamChunkIter<R> {
    type Item = Result<Chunk, Mp4Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.static_codec_emitted {
            self.static_codec_emitted = true;
            return Some(build_codec_chunk(&self.entity_path, self.mapped_codec));
        }

        // Skip ranges whose samples were all unloaded (no rows to emit); only
        // stop once we have a non-empty chunk, an error, or run out of ranges.
        while self.cursor < self.ranges.len() {
            let range = self.ranges[self.cursor].clone();
            self.cursor += 1;

            match build_gop_chunk(
                &mut self.reader,
                &self.desc,
                self.timescale,
                &self.timeline_name,
                self.timeline_type,
                self.has_b_frames,
                &self.entity_path,
                range,
            ) {
                Ok(Some(chunk)) => return Some(Ok(chunk)),
                Ok(None) => {} // empty range — try the next one
                Err(err) => return Some(Err(err)),
            }
        }
        None
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

/// Build a single chunk for `range`, or `Ok(None)` if every sample in the range
/// was unloaded (nothing to emit — the caller skips it).
#[expect(clippy::too_many_arguments)]
fn build_gop_chunk<R: Read + Seek>(
    reader: &mut R,
    desc: &VideoDataDescription,
    timescale: re_video::Timescale,
    timeline_name: &str,
    timeline_type: TimeType,
    has_b_frames: bool,
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
        let byte_range = span.range_usize();
        reader.seek(SeekFrom::Start(byte_range.start as u64))?;
        sample_bytes.resize(byte_range.len(), 0);
        reader.read_exact(&mut sample_bytes)?;

        let chunk = meta
            .get(&|_src| sample_bytes.as_slice(), sample_idx)
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
    // With no B-frames the samples are in PTS order, so the column is sorted.
    // With B-frames the PTS in decode order is not monotonic, so we mark it
    // unsorted outright — `Some(false)` rather than `None` so the store skips
    // the O(n) sortedness scan it would otherwise run on an unknown column.
    let is_sorted = Some(!has_b_frames);
    let time_column = TimeColumn::new(
        is_sorted,
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
