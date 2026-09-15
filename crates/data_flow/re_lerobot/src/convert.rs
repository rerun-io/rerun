//! `LeRobot` → Rerun chunk conversion: the per-kind executors and the chunk builders they
//! share.

use std::collections::HashSet;
use std::path::Path;

use arrow::array::{ArrayRef, ListArray};
use arrow::buffer::ScalarBuffer;
use itertools::{Either, Itertools as _};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::ArrowArray as _;
use re_chunk::{
    Chunk, ChunkId, ComponentIdentifier, EntityPath, RowId, TimeColumn, TimeInt, TimePoint,
    Timeline, TimelineName,
};
use re_log_types::TimeType;
use re_mp4_reader::{Mode, Mp4Config, Mp4Error, Mp4TranscodeOptions, TimeWindow, load_mp4};
use re_parquet::{ColumnGrouping, IndexColumn, IndexType, ParquetConfig, TimeUnit};
use re_sdk_types::archetypes::{AssetVideo, SeriesLines, TextDocument, VideoFrameReference};
use re_sdk_types::datatypes::VideoTimestamp;

use crate::config::LeRobotConfig;
use crate::dataset::{EpisodeAddress, Tasks, VideoSource};
use crate::emits::{
    Emits, FRAME_INDEX_COLUMN, LANGUAGE_PERSISTENT_COLUMN, LanguageEmit, TIMESTAMP_COLUMN,
    TabularEmit, TabularEmitKind, VideoEmit, entity_path,
};
use crate::error::LeRobotError;
use crate::language::resolve_language_tracks;
use crate::lenses::is_row_aligned_unit_list;

// ---------------------------------------------------------------------------
// Episode execution

/// The timeline an episode is placed on: a sequence timeline fed by `frame_index` when
/// the data files carry that column, otherwise a duration timeline fed by `timestamp`.
#[derive(Clone, Copy)]
struct EpisodeTimeline {
    /// The parquet column the timeline reads.
    column: &'static str,

    timeline: Timeline,
}

fn episode_timeline(has_frame_index: bool, timeline_name: Option<TimelineName>) -> EpisodeTimeline {
    if has_frame_index {
        EpisodeTimeline {
            column: FRAME_INDEX_COLUMN,
            timeline: Timeline::new_sequence(
                timeline_name.unwrap_or_else(|| FRAME_INDEX_COLUMN.into()),
            ),
        }
    } else {
        EpisodeTimeline {
            column: TIMESTAMP_COLUMN,
            timeline: Timeline::new_duration(
                timeline_name.unwrap_or_else(|| TIMESTAMP_COLUMN.into()),
            ),
        }
    }
}

/// Turn one episode's emits into chunks, as returned by [`crate::LeRobotDataset::stream`].
pub fn execute(
    emits: Emits,
    address: &EpisodeAddress,
    tasks: &Tasks,
    config: &LeRobotConfig,
    has_frame_index: bool,
    fps: f64,
) -> Result<impl Iterator<Item = Result<Chunk, LeRobotError>> + use<>, LeRobotError> {
    let episode_timeline = episode_timeline(has_frame_index, config.timeline_name);
    let timeline = episode_timeline.timeline;
    let prefix = config.entity_path_prefix.clone();

    let tabular = tabular_chunks(emits.tabular, address, tasks, episode_timeline, &prefix)?;
    let language = language_chunks(emits.language, address, episode_timeline, &prefix, fps)?;
    let videos = emits
        .videos
        .into_iter()
        .flat_map(move |emit| video_chunks(&emit, timeline));

    Ok(std::iter::chain(
        tabular,
        std::iter::chain(language, videos),
    ))
}

// ---------------------------------------------------------------------------
// The tabular tier: projected parquet reads, chunked by re_parquet
fn tabular_chunks(
    emits: Vec<TabularEmit>,
    address: &EpisodeAddress,
    tasks: &Tasks,
    episode_timeline: EpisodeTimeline,
    prefix: &EntityPath,
) -> Result<impl Iterator<Item = Result<Chunk, LeRobotError>> + use<>, LeRobotError> {
    if emits.is_empty() {
        return Ok(Either::Left(std::iter::empty()));
    }

    // The names come from `info.json` alone, so these static chunks cost no data read.
    let series_names: Vec<(EntityPath, Vec<String>)> = emits
        .iter()
        .filter_map(|emit| match &emit.kind {
            TabularEmitKind::Scalars { names, .. } if !names.is_empty() => {
                Some((emit.entity.clone(), names.clone()))
            }
            _ => None,
        })
        .collect();

    let lenses = crate::lenses::build_lenses(&emits, tasks)?;
    let runtime = re_lenses::default_runtime();
    let columns = emits.into_iter().map(|emit| emit.column).collect();
    let parquet_config = projected_read(columns, address, episode_timeline);
    let chunks = open_tabular(&address.data_file, &parquet_config, prefix)?;

    Ok(Either::Right(std::iter::chain(
        series_names
            .into_iter()
            .map(|(entity, names)| build_series_names_chunk(&entity, &names)),
        chunks.flat_map(move |item| match item {
            Ok(chunk) => Either::Right(apply_lenses(&chunk, &lenses, &runtime).into_iter()),
            Err(err) => Either::Left(std::iter::once(Err(err))),
        }),
    )))
}

/// The language tier: one projected read of the episode's annotation columns, fanned out
/// to per-entity text tracks.
/// TODO(RR-5493): implement proper handling of persistent language annotations.
/// this would make language chunks move to tabular chunks.
fn language_chunks(
    emits: Vec<LanguageEmit>,
    address: &EpisodeAddress,
    episode_timeline: EpisodeTimeline,
    prefix: &EntityPath,
    fps: f64,
) -> Result<impl Iterator<Item = Result<Chunk, LeRobotError>> + use<>, LeRobotError> {
    if emits.is_empty() {
        return Ok(Either::Left(std::iter::empty()));
    }

    let timeline = episode_timeline.timeline;
    let columns = emits.iter().map(|emit| emit.column.clone()).collect();
    let parquet_config = projected_read(columns, address, episode_timeline);
    let chunks = open_tabular(&address.data_file, &parquet_config, prefix)?;

    // Persistent language annotations are broadcast to every row, so one record batch
    // carries them all; later batches of the same column must not repeat them. A column
    // that fails also lands here, so it errors once, not once per batch.
    let mut columns_done: HashSet<String> = HashSet::new();

    Ok(Either::Right(chunks.flat_map(move |item| {
        let chunk = match item {
            Ok(chunk) => chunk,
            Err(err) => return Either::Left(std::iter::once(Err(err))),
        };
        let tracks: Vec<Result<LanguageTrack, LeRobotError>> = emits
            .iter()
            .flat_map(|emit| {
                match language_tracks(
                    &chunk,
                    &emit.prefix,
                    &emit.column,
                    timeline,
                    fps,
                    &mut columns_done,
                ) {
                    Ok(tracks) => Either::Left(tracks.into_iter().map(Ok)),
                    Err(err) => Either::Right(std::iter::once(Err(err))),
                }
            })
            .collect();
        Either::Right(tracks.into_iter().map(move |track| {
            let (entity, rows) = track?;
            build_text_chunk(&entity, &rows, &timeline)
        }))
    })))
}

/// The projected read of one set of the episode's tabular columns: only those columns
/// (the index column is always kept), and only the episode's row span of the shared v3
/// data file.
fn projected_read(
    columns: Vec<String>,
    address: &EpisodeAddress,
    episode_timeline: EpisodeTimeline,
) -> ParquetConfig {
    ParquetConfig {
        column_grouping: ColumnGrouping::Individual,
        index_columns: vec![IndexColumn {
            name: episode_timeline.column.to_owned(),
            index_type: match episode_timeline.timeline.typ() {
                TimeType::Sequence => IndexType::Sequence,
                // LeRobot timestamps are fractional seconds since episode start.
                _ => IndexType::Duration(TimeUnit::Seconds),
            },
            output_name: Some(episode_timeline.timeline.name().to_string()),
        }],
        columns: Some(columns),
        row_window: address.rows,
        ..Default::default()
    }
}

/// Open the episode's projected, row-windowed parquet read, with the file path attached
/// to every error.
fn open_tabular(
    data_file: &Path,
    parquet_config: &ParquetConfig,
    prefix: &EntityPath,
) -> Result<impl Iterator<Item = Result<Chunk, LeRobotError>> + use<>, LeRobotError> {
    let path = data_file.to_path_buf();
    let chunks = re_parquet::load_parquet(data_file, parquet_config, prefix)
        .map_err(|source| LeRobotError::episode_data_read(source, &path))?
        // The file's key-value metadata (a pandas schema) is not Rerun data; the importer
        // owns the recording's properties entity.
        .filter(
            |item| !matches!(item, Ok(chunk) if chunk.entity_path() == &EntityPath::properties()),
        );
    Ok(chunks
        .map(move |item| item.map_err(|source| LeRobotError::episode_data_read(source, &path))))
}

/// The named data column `re_parquet` packed into this chunk, unwrapped back to the raw
/// parquet column values (row-aligned with the chunk's time column).
///
/// The values buffer is only taken when it is row-aligned, so a misaligned column skips
/// rather than misattributing values to rows.
fn column_values(chunk: &Chunk, column: &str) -> Option<ArrayRef> {
    let identifier = ComponentIdentifier::try_new(column).ok()?;
    let list = &chunk.components().get(identifier)?.list_array;
    is_row_aligned_unit_list(list).then(|| list.values().clone())
}

/// The chunk's time values on the episode timeline.
fn chunk_times(chunk: &Chunk, timeline: Timeline) -> &[i64] {
    chunk
        .timelines()
        .get(timeline.name())
        .map_or(&[], |column| column.times_raw())
}

/// Apply the feature lenses to one raw column chunk.
///
/// A failed lens still surfaces its partial chunk (the columns that succeeded) ahead of
/// the error.
/// TODO(RR-5278): `Lenses::apply()` borrows the chunk
fn apply_lenses(
    chunk: &Chunk,
    lenses: &re_lenses::Lenses,
    runtime: &re_lenses::Runtime,
) -> Vec<Result<Chunk, LeRobotError>> {
    lenses
        .apply(chunk, runtime)
        .flat_map(|result| match result {
            Ok(chunk) => Either::Left(std::iter::once(Ok(chunk))),
            Err(err) => {
                let details = err.errors().map(ToString::to_string).join(", ");
                let partial = err.partial_chunk();
                Either::Right(std::iter::chain(
                    partial.map(Ok),
                    std::iter::once(Err(LeRobotError::Lens(details))),
                ))
            }
        })
        .collect()
}

/// A static `SeriesLines` chunk naming a scalar feature's per-element series, from the
/// feature's `names` metadata.
fn build_series_names_chunk(entity: &EntityPath, names: &[String]) -> Result<Chunk, LeRobotError> {
    let series = SeriesLines::update_fields().with_names(names.iter().cloned());
    Ok(Chunk::builder(entity.clone())
        .with_archetype(RowId::new(), TimePoint::default(), &series)
        .build()?)
}

/// One output entity's annotation rows, placed on the episode timeline.
type LanguageTrack = (EntityPath, Vec<(TimeInt, String)>);

/// A language-column chunk's annotation rows per output entity, placed on the episode
/// timeline. Empty when the column is already done, or its rows carry nothing.
fn language_tracks(
    chunk: &Chunk,
    prefix: &EntityPath,
    column: &str,
    timeline: Timeline,
    fps: f64,
    columns_done: &mut HashSet<String>,
) -> Result<Vec<LanguageTrack>, LeRobotError> {
    if columns_done.contains(column) {
        return Ok(Vec::new());
    }
    let is_persistent = column == LANGUAGE_PERSISTENT_COLUMN;

    let Some(values) = column_values(chunk, column) else {
        return Ok(Vec::new());
    };
    let Some(list) = values.downcast_array_ref::<ListArray>() else {
        columns_done.insert(column.to_owned());
        return Err(LeRobotError::InvalidLanguageColumn {
            column: column.to_owned(),
            datatype: values.data_type().clone(),
        });
    };

    // A persistent annotation's emission timestamp maps straight to the episode timeline:
    // a frame position via the dataset fps on a sequence timeline, nanoseconds on a
    // duration timeline. LeRobot rows sit on the fps grid, so this matches placing them
    // on the row with that timestamp.
    let to_time = move |ts: f64| -> i64 {
        match timeline.typ() {
            #[expect(clippy::cast_possible_truncation)]
            TimeType::Sequence => (ts * fps).round() as i64,
            _ => re_log_types::Duration::from_secs(ts).as_nanos(),
        }
    };
    let times = chunk_times(chunk, timeline);

    let tracks: Vec<LanguageTrack> = resolve_language_tracks(column, list, &to_time)
        .into_iter()
        .map(|(sub_entity, rows)| {
            let rows = if is_persistent {
                // Persistent rows already carry times (via `to_time`).
                rows
            } else {
                // Event rows carry batch-relative frame positions; place them at the
                // batch's actual times.
                rows.into_iter()
                    .map(|(frame, text)| (row_time(frame, times), text))
                    .collect()
            };
            (entity_path(prefix, &sub_entity), rows)
        })
        .collect();

    if is_persistent && !tracks.is_empty() {
        columns_done.insert(column.to_owned());
    }

    Ok(tracks)
}

/// Place a row/frame position at the episode's actual time on its timeline.
///
/// On a `frame_index` sequence timeline this is typically the identity; on a `timestamp`
/// duration timeline it maps the position to the row's timestamp.
fn row_time(row: TimeInt, times: &[i64]) -> TimeInt {
    usize::try_from(row.as_i64())
        .ok()
        .and_then(|index| times.get(index))
        .map_or(row, |time| TimeInt::new_temporal(*time))
}

// ---------------------------------------------------------------------------
// The video tier: each video streams from its own container

/// Stream one video emit's chunks from its own container.
fn video_chunks(
    emit: &VideoEmit,
    timeline: Timeline,
) -> impl Iterator<Item = Result<Chunk, LeRobotError>> + use<> {
    match &emit.source {
        VideoSource::Stream { file, window, fps } => Either::Left(stream_video_chunks(
            &emit.entity,
            file,
            *window,
            *fps,
            timeline,
        )),
        VideoSource::Asset { file } => Either::Right(
            match std::fs::read(file).map_err(|err| LeRobotError::io(err, file)) {
                Ok(contents) => {
                    Either::Left(build_video_asset_chunks(&emit.entity, contents, timeline))
                }
                Err(err) => Either::Right(std::iter::once(Err(err))),
            },
        ),
    }
}

/// Stream one episode's slice of an mp4 through [`re_mp4_reader`], one GOP resident at
/// a time, with sample timestamps placed on the episode's timeline.
fn stream_video_chunks(
    entity: &EntityPath,
    file: &Path,
    window: Option<TimeWindow>,
    fps: f64,
    timeline: Timeline,
) -> impl Iterator<Item = Result<Chunk, LeRobotError>> + use<> {
    // LeRobot episode windows land on GOP boundaries (episodes are recorded whole and
    // concatenated), so the reader serves them directly, no ffmpeg needed. A rare
    // misaligned window smart-cuts its first GOP, which requires ffmpeg.
    let config = Mp4Config {
        mode: Mode::Stream {
            chunk_by_gop: true,
            transcode: Mp4TranscodeOptions::default(),
            time_window: window,
        },
        timeline_name: *timeline.name(),
        timeline_type: TimeType::DurationNs,
    };

    let file = file.to_path_buf();
    let iter = match load_mp4(&file, &config, entity) {
        Ok(iter) => iter,
        Err(err) => return Either::Left(std::iter::once(Err(video_error(err, &file)))),
    };

    // The reader emits sample timestamps as durations (window-relative, rebased to zero);
    // a sequence timeline gets them retagged to frame indices via the dataset's fps.
    let retag_to_sequence = timeline.typ() == TimeType::Sequence;
    Either::Right(iter.map(move |item| match item {
        Ok(chunk) if retag_to_sequence => retime_to_sequence(chunk, timeline, fps),
        Ok(chunk) => Ok(chunk),
        Err(err) => Err(video_error(err, &file)),
    }))
}

/// Replace a chunk's duration time column with frame indices (`round(pts * fps)`).
/// TODO(RR-5686): delete once `re_mp4_reader` can emit sample indices directly.
fn retime_to_sequence(chunk: Chunk, timeline: Timeline, fps: f64) -> Result<Chunk, LeRobotError> {
    let Some(times) = chunk.timelines().get(timeline.name()) else {
        return Ok(chunk);
    };

    #[expect(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    let frames: arrow::buffer::ScalarBuffer<i64> = times
        .times_raw()
        .iter()
        .map(|pts_ns| ((*pts_ns as f64) * fps / 1e9).round() as i64)
        .collect();
    let time_column = TimeColumn::new(None, timeline, frames);

    Ok(Chunk::from_native_row_ids(
        chunk.id(),
        chunk.entity_path().clone(),
        None,
        chunk.row_ids_slice(),
        std::iter::once((*timeline.name(), time_column)).collect(),
        chunk.components().clone(),
    )?)
}

/// Attach the ffmpeg remedy to transcode failures; other reader errors pass through.
fn video_error(source: Mp4Error, path: &Path) -> LeRobotError {
    let path = path.to_path_buf();
    if source.is_ffmpeg_related() {
        LeRobotError::VideoTranscode { source, path }
    } else {
        LeRobotError::Video { source, path }
    }
}

// ---------------------------------------------------------------------------
// Chunk builders

/// One `TextDocument` chunk from resolved (time, text) rows.
pub fn build_text_chunk(
    entity: &EntityPath,
    rows: &[(TimeInt, String)],
    timeline: &Timeline,
) -> Result<Chunk, LeRobotError> {
    let mut chunk = Chunk::builder(entity.clone());
    let mut row_id = RowId::new();
    for (time, text) in rows {
        let timepoint = TimePoint::default().with(*timeline, *time);
        chunk = chunk.with_archetype(row_id, timepoint, &TextDocument::new(text.clone()));
        row_id = row_id.next();
    }
    Ok(chunk.build()?)
}

/// v2 video: a static [`AssetVideo`] chunk plus, when frame timestamps can be read from the
/// container, a [`VideoFrameReference`] chunk aligning video frames with the episode timeline.
fn build_video_asset_chunks(
    entity: &EntityPath,
    contents: Vec<u8>,
    timeline: Timeline,
) -> impl Iterator<Item = Result<Chunk, LeRobotError>> + use<> {
    match build_video_asset(entity, contents, timeline) {
        Ok((asset_chunk, frame_ref)) => Either::Left(std::iter::chain(
            std::iter::once(Ok(asset_chunk)),
            frame_ref.map(Ok),
        )),
        Err(err) => Either::Right(std::iter::once(Err(err))),
    }
}

/// The static asset chunk and, when frame timestamps can be read from the container, the
/// frame reference chunk.
///
/// The frame times come from the container itself, not from the episode's parquet rows:
/// frame indices on a sequence timeline, the mp4's own timestamps on a duration timeline.
fn build_video_asset(
    entity: &EntityPath,
    contents: Vec<u8>,
    timeline: Timeline,
) -> Result<(Chunk, Option<Chunk>), LeRobotError> {
    let video_asset = AssetVideo::new(contents);
    // Static asset chunk kept separate — it can be large.
    let asset_chunk = Chunk::builder(entity.clone())
        .with_archetype(RowId::new(), TimePoint::default(), &video_asset)
        .build()?;

    let frame_ref = match video_asset.read_frame_timestamps_nanos() {
        Ok(timestamps) => {
            let timestamps: ScalarBuffer<i64> = timestamps.into();
            let times: ScalarBuffer<i64> = match timeline.typ() {
                #[expect(clippy::cast_possible_wrap)]
                TimeType::Sequence => (0..timestamps.len() as i64).collect(),
                _ => timestamps.clone(),
            };
            let video_timestamps = timestamps
                .iter()
                .copied()
                .map(VideoTimestamp::from_nanos)
                .collect::<Vec<_>>();
            let column = VideoFrameReference::update_fields()
                .with_many_timestamp(video_timestamps)
                .columns_of_unit_batches()?;
            let time_column = TimeColumn::new(None, timeline, times);
            Some(Chunk::from_auto_row_ids(
                ChunkId::new(),
                entity.clone(),
                std::iter::once((*timeline.name(), time_column)).collect(),
                column.collect(),
            )?)
        }
        Err(err) => {
            re_log::warn_once!("Failed to read frame timestamps from {entity} video: {err}");
            None
        }
    };

    Ok((asset_chunk, frame_ref))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::Int64Array;
    use arrow::datatypes::Field;
    use re_sdk_types::ComponentDescriptor;

    use super::*;

    /// A single-column chunk shaped like `re_parquet` output: each row wraps one element
    /// of the raw column array in a one-element list.
    fn column_chunk(column: &str, values: ArrayRef) -> Chunk {
        let field = Arc::new(Field::new("item", values.data_type().clone(), true));
        let offsets =
            arrow::buffer::OffsetBuffer::from_lengths(std::iter::repeat_n(1, values.len()));
        let list = ListArray::try_new(field, offsets, values, None).unwrap();

        let timeline = Timeline::new_sequence("frame_index");
        let times = TimeColumn::new(
            None,
            timeline,
            (0..i64::try_from(list.len()).unwrap())
                .collect::<Vec<_>>()
                .into(),
        );

        let descriptor =
            ComponentDescriptor::partial(ComponentIdentifier::try_new(column).unwrap());
        let components: re_chunk::ChunkComponents = std::iter::once((descriptor, list)).collect();
        Chunk::from_auto_row_ids(
            ChunkId::new(),
            EntityPath::from(format!("/{column}")),
            std::iter::once((*timeline.name(), times)).collect(),
            components,
        )
        .unwrap()
    }

    /// A language column whose rows are not lists of annotation rows errors on the first
    /// batch and stays quiet on later ones.
    #[test]
    fn a_malformed_language_column_errors_once() {
        let values: ArrayRef = Arc::new(Int64Array::from(vec![1, 2]));
        let chunk = column_chunk("language_events", values);
        let prefix = EntityPath::from("/episode");
        let timeline = Timeline::new_sequence("frame_index");
        let mut columns_done = HashSet::new();

        let err = language_tracks(
            &chunk,
            &prefix,
            "language_events",
            timeline,
            30.0,
            &mut columns_done,
        )
        .unwrap_err();
        assert!(matches!(err, LeRobotError::InvalidLanguageColumn { .. }));

        let tracks = language_tracks(
            &chunk,
            &prefix,
            "language_events",
            timeline,
            30.0,
            &mut columns_done,
        )
        .unwrap();
        assert!(tracks.is_empty());
    }

    /// A row position maps to the episode's actual time: the identity on a 0-based
    /// sequence timeline, the row's timestamp on a duration timeline.
    #[test]
    fn text_rows_land_on_the_episode_times() {
        let frame_times = [0_i64, 1, 2];
        let duration_times = [0_i64, 33_333_333, 66_666_666];

        for (times, expected) in [(frame_times, 1), (duration_times, 33_333_333)] {
            assert_eq!(
                row_time(TimeInt::new_temporal(1), &times),
                TimeInt::new_temporal(expected)
            );
        }

        // Out-of-range positions (persistent language fallback) keep their value.
        assert_eq!(
            row_time(TimeInt::new_temporal(7), &frame_times),
            TimeInt::new_temporal(7)
        );
    }

    /// The names chunk is static and lands on the scalar feature's entity.
    #[test]
    fn series_names_build_a_static_chunk() {
        let entity = EntityPath::from("/action");
        let names = ["shoulder".to_owned(), "elbow".to_owned()];
        let chunk = build_series_names_chunk(&entity, &names).unwrap();

        assert!(chunk.is_static());
        assert_eq!(chunk.entity_path(), &entity);
        assert!(
            chunk
                .components()
                .contains_component(SeriesLines::descriptor_names().component)
        );
    }
}
