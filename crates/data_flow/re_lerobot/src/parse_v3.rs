//! Pure parser for v3 `LeRobot` datasets into the version-free [`DatasetData`].
//!
//! # `LeRobot` v3 dataset format
//!
//! The dataset follows a standardized directory layout, typically organized as follows:
//!
//! ```text
//! .
//! ├── README.md
//! ├── data/
//! │   └── chunk-000/
//! │       ├── file-000.parquet
//! │       ├── file-001.parquet
//! │       └── …
//! ├── meta/
//! │   ├── episodes/
//! │   │   └── chunk-000/
//! │   │       ├── file-000.parquet
//! │   │       └── …
//! │   ├── tasks.parquet
//! │   ├── subtasks.parquet
//! │   ├── stats.json
//! │   └── info.json
//! └── videos/
//!     └── observation.image/
//!         └── chunk-000/
//!             ├── file-000.mp4
//!             └── …
//! ```
//!
//! ## File layout
//!
//! - `data/`: Episode data in Parquet format; one file holds the rows of several episodes.
//! - `meta/`: Contains metadata files:
//!   - `episodes/`: Per-episode addresses: data file, row range, and video time windows.
//!   - `info.json`: General dataset metadata (features, fps, number of episodes, etc.).
//!   - `tasks.parquet`: The task text for each `task_index`.
//!   - `subtasks.parquet`: Optional subtask text for each `subtask_index`.
//!   - `stats.json`: Summary statistics of dataset features.
//! - `videos/`: Optional per-feature videos; one file holds the frames of several episodes.
//!
//! Each episode is addressed through the `meta/episodes` tables: which shared data file
//! holds it, its row range (dataset-wide, rebased to the file at parse time), and its
//! time window within each video file.

use crate::dataset::{
    DatasetData, EpisodeAddress, EpisodeIndex, SubtaskIndex, TaskIndex, Tasks, VideoSource,
};
use crate::emits::{FRAME_INDEX_COLUMN, LEROBOT_DATASET_IGNORED_COLUMNS, TIMESTAMP_COLUMN};
use crate::error::LeRobotError;
use crate::features::{DType, Feature, FeatureKey, normalize_string_array};
use crate::language::timestamps_as_f64;
use crate::version::LeRobotDatasetVersion;
use crate::{LeRobotDiagnostic, LeRobotDiagnostics};

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ahash::HashMap;
use arrow::array::{Float64Array, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType as ArrowDataType, Field as ArrowField, SchemaRef};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use re_mp4_reader::TimeWindow;
use serde::{Deserialize, Serialize};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::ArrowArray as _;

/// Parse the v3 dataset at `path` into the version-free model.
///
/// Resolves every episode's data file, row range, and video sources, and validates them
/// against the data files' parquet footers, so every parsed episode streams under a
/// valid config.
///
/// Defects are skipped at the finest scope they touch: a bad episode or feature is
/// dropped with a warning; only dataset-wide defects fail the parse.
pub fn parse(
    path: &Path,
    diagnostics: &mut LeRobotDiagnostics,
) -> Result<DatasetData, LeRobotError> {
    let mut metadata =
        LeRobotDatasetV3Metadata::load_from_directory(path.join("meta"), diagnostics)?;
    validate_dataset_info(&metadata.info)?;
    validate_video_fps(&mut metadata.info.features, diagnostics);

    let video_features: Vec<(&FeatureKey, &Feature)> = metadata
        .info
        .features
        .iter()
        .filter(|(_, feature)| feature.dtype == DType::Video)
        .collect();
    if !video_features.is_empty() && metadata.info.video_path.is_none() {
        for (key, _) in &video_features {
            diagnostics.add(LeRobotDiagnostic::SkippedFeature {
                feature: (*key).clone(),
                err: LeRobotError::MissingDatasetInfo("`video_path` in `meta/info.json`".into()),
            });
        }
    }

    let file_starts = data_file_starts(path, &metadata.info, metadata.episodes.values());

    let mut episodes = BTreeMap::new();
    for (&index, episode_data) in &metadata.episodes {
        match resolve_episode(
            path,
            &metadata.info,
            episode_data,
            &video_features,
            &file_starts,
            diagnostics,
        ) {
            Ok(address) => {
                episodes.insert(index, address);
            }
            Err(err) => {
                diagnostics.add(LeRobotDiagnostic::SkippedEpisode {
                    episode: index,
                    err,
                });
            }
        }
    }

    let mut features = metadata.info.features;
    let schema =
        validate_against_footers(&mut episodes, &mut features, diagnostics).ok_or_else(|| {
            LeRobotError::NoEpisodes {
                path: path.to_path_buf(),
            }
        })?;

    // if no episodes are left after validating against footers
    if episodes.is_empty() {
        return Err(LeRobotError::NoEpisodes {
            path: path.to_path_buf(),
        });
    }

    // The episode timeline must follow the schema, not the feature list, which can
    // name a column the files lack.
    let has_frame_index = schema.index_of(FRAME_INDEX_COLUMN).is_ok();

    Ok(DatasetData {
        version: LeRobotDatasetVersion::V3,
        features,
        episodes,
        tasks: Tasks {
            tasks: metadata.tasks,
            subtasks: metadata.subtasks,
        },
        fps: f64::from(metadata.info.fps),
        has_frame_index,
    })
}

/// Validate episodes and features against their data files' parquet footers (row count
/// and schema).
///
/// Returns the schema shared by every data file, or `None` when no file is readable
fn validate_against_footers(
    episodes: &mut BTreeMap<EpisodeIndex, EpisodeAddress>,
    features: &mut BTreeMap<FeatureKey, Feature>,
    diagnostics: &mut LeRobotDiagnostics,
) -> Option<SchemaRef> {
    // One footer read per unique data file; `None` marks a file whose episodes all drop.
    let mut footers: BTreeMap<PathBuf, Option<(u64, SchemaRef)>> = BTreeMap::new();
    for address in episodes.values() {
        if !footers.contains_key(&address.data_file) {
            let footer = match read_data_footer(&address.data_file) {
                Ok(footer) => Some(footer),
                Err(err) => {
                    diagnostics.add(LeRobotDiagnostic::SkippedDataFile {
                        path: address.data_file.clone(),
                        err,
                    });
                    None
                }
            };
            footers.insert(address.data_file.clone(), footer);
        }
    }

    episodes.retain(|index, address| {
        let Some(Some((num_rows, _))) = footers.get(&address.data_file) else {
            return false; // the file-scoped warning already covers this episode
        };
        let in_range = address.rows.is_none_or(|rows| rows.end() <= *num_rows);
        if !in_range {
            diagnostics.add(LeRobotDiagnostic::SkippedEpisode {
                episode: *index,
                err: LeRobotError::InvalidDatasetInfo(format!(
                    "its row range ends beyond the {num_rows} rows of its data file\nFile path: {}",
                    address.data_file.display()
                )),
            });
        }
        in_range
    });

    // The v3 spec gives every data file the same schema; validate features against the
    // first readable one.
    let Some((_, schema)) = footers.values().flatten().next() else {
        return None; // no readable file: `episodes` is empty and open() fails
    };
    features.retain(|key, feature| {
        if !reads_column(key, feature) {
            return true;
        }
        let Ok(field) = schema.field_with_name(key.as_str()) else {
            diagnostics.add(LeRobotDiagnostic::SkippedFeature {
                feature: key.clone(),
                err: LeRobotError::MissingDatasetInfo(format!("column `{key}` in the data files")),
            });
            return false;
        };
        // Image is the only dtype validated against its on-disk shape here: scalar
        // columns cast to Float64 at execute, so any numeric column fits, and the task
        // joins error at execute when their column is not `Int64`.
        if feature.dtype == DType::Image && !is_image_bytes_column(field) {
            diagnostics.add(LeRobotDiagnostic::SkippedFeature {
                feature: key.clone(),
                err: LeRobotError::InvalidDatasetInfo(format!(
                    "column type `{}` cannot feed a `{:?}` feature",
                    field.data_type(),
                    feature.dtype
                )),
            });
            return false;
        }
        true
    });

    Some(schema.clone())
}

/// Read one data file's parquet footer: row count and schema.
fn read_data_footer(path: &Path) -> Result<(u64, SchemaRef), LeRobotError> {
    let file = File::open(path).map_err(|err| LeRobotError::io(err, path))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|source| {
        LeRobotError::DataFileFooter {
            source,
            path: path.to_path_buf(),
        }
    })?;
    let schema = builder.schema().clone();
    if schema.index_of(FRAME_INDEX_COLUMN).is_err() && schema.index_of(TIMESTAMP_COLUMN).is_err() {
        return Err(LeRobotError::MissingTimeline {
            path: path.to_path_buf(),
        });
    }
    let num_rows = u64::try_from(builder.metadata().file_metadata().num_rows()).unwrap_or(0);
    Ok((num_rows, schema))
}

/// Does this feature's emit read a column of the episode data file?
fn reads_column(key: &FeatureKey, feature: &Feature) -> bool {
    if LEROBOT_DATASET_IGNORED_COLUMNS.contains(&key.as_str()) {
        return false;
    }
    match feature.dtype {
        DType::Float32 | DType::Float64 | DType::Image | DType::String => true,
        DType::Int64 => key.as_str() == "task_index" || key.as_str() == "subtask_index",
        // Video reads its own container, language columns are resolved tolerantly at
        // execute time, and the rest emit nothing.
        DType::Video | DType::Language | DType::Bool | DType::Int16 | DType::Unknown => false,
    }
}

/// Is `field` the `struct<bytes: binary>` column an image feature's emit reads?
///
/// Only what the schema can answer about an image column; everything finer is left to
/// execution, which warns rather than errors.
fn is_image_bytes_column(field: &ArrowField) -> bool {
    matches!(
        field.data_type(),
        ArrowDataType::Struct(fields)
            if fields.iter().any(|field| {
                field.name() == "bytes" && field.data_type() == &ArrowDataType::Binary
            })
    )
}

/// The dataset-scoped checks: defects that make every episode meaningless fail the parse.
fn validate_dataset_info(info: &LeRobotDatasetV3Info) -> Result<(), LeRobotError> {
    let version = info.codebase_version.trim_start_matches('v');
    if version.split('.').next() != Some("3") {
        return Err(LeRobotError::InvalidDatasetInfo(format!(
            "unrecognized `codebase_version` `{}` for a v3-layout dataset",
            info.codebase_version
        )));
    }
    if info.features.is_empty() {
        return Err(LeRobotError::InvalidDatasetInfo(
            "the `features` map is empty".to_owned(),
        ));
    }
    if !info.fps.is_finite() || info.fps <= 0.0 {
        return Err(LeRobotError::InvalidDatasetInfo(format!(
            "`fps` must be positive, got {}",
            info.fps
        )));
    }
    Ok(())
}

/// Clear invalid per-feature `video.fps` values (with a warning), so episode resolution
/// can trust the field.
fn validate_video_fps(
    features: &mut BTreeMap<FeatureKey, Feature>,
    diagnostics: &mut LeRobotDiagnostics,
) {
    for (key, feature) in features.iter_mut() {
        if let Some(feature_info) = &mut feature.info
            && let Some(fps) = feature_info.video_fps
            && (!fps.is_finite() || fps <= 0.0)
        {
            diagnostics.add(LeRobotDiagnostic::MetadataWarning {
                reason: format!(
                    "Ignoring invalid `video.fps` {fps} of feature `{key}`; using the dataset fps"
                ),
            });
            feature_info.video_fps = None;
        }
    }
}

/// The first dataset-global row of every data file: the smallest `dataset_from_index`
/// among the episodes that share it.
///
/// `dataset_from_index`/`dataset_to_index` address rows over the whole concatenated
/// dataset (the `index` column), while reads slice within one file, so every episode's
/// range is rebased by its file's start. Computed over all metadata rows with a valid
/// range, so dropping one episode later never shifts its siblings.
fn data_file_starts<'a>(
    path: &Path,
    info: &LeRobotDatasetV3Info,
    episodes: impl Iterator<Item = &'a LeRobotEpisodeV3MetaData>,
) -> HashMap<PathBuf, u64> {
    let mut starts: HashMap<PathBuf, u64> = HashMap::default();
    for episode_data in episodes {
        let Some(from) = Option::zip(
            episode_data.dataset_from_index,
            episode_data.dataset_to_index,
        )
        .filter(|(from, to)| from < to)
        .map(|(from, _)| from) else {
            continue;
        };
        let data_file = path.join(info.episode_data_path(episode_data));
        starts
            .entry(data_file)
            .and_modify(|start| *start = (*start).min(from))
            .or_insert(from);
    }
    starts
}

/// Resolve one episode's addresses, or the episode-scoped error that gets it dropped.
fn resolve_episode(
    path: &Path,
    info: &LeRobotDatasetV3Info,
    episode_data: &LeRobotEpisodeV3MetaData,
    video_features: &[(&FeatureKey, &Feature)],
    file_starts: &HashMap<PathBuf, u64>,
    diagnostics: &mut LeRobotDiagnostics,
) -> Result<EpisodeAddress, LeRobotError> {
    // Existence is not checked here: the footer pass validates each unique data file
    // once, so a missing file warns once instead of once per episode.
    let data_file = path.join(info.episode_data_path(episode_data));

    // Every episode with a valid range contributed to its file's start, so the
    // subtraction cannot underflow.
    let file_start = file_starts.get(&data_file).copied().unwrap_or(0);
    let rows = Option::zip(
        episode_data.dataset_from_index,
        episode_data.dataset_to_index,
    )
    .filter(|(from, to)| from < to)
    .map(|(from, to)| re_span::Span::from_start_end(from - file_start, to - file_start))
    .ok_or_else(|| {
        LeRobotError::MissingDatasetInfo(
            "a valid `dataset_from_index`..`dataset_to_index` row range in `meta/episodes`".into(),
        )
    })?;

    let mut videos = HashMap::default();
    if info.video_path.is_some() {
        for &(key, feature) in video_features {
            // Feature-scoped: an unresolvable video drops that emit for this episode only.
            let file = match info.video_path(key, episode_data) {
                Ok(file) => path.join(file),
                Err(err) => {
                    diagnostics.add(LeRobotDiagnostic::FailedFeature {
                        episode: episode_data.episode_index,
                        feature: Some(key.clone()),
                        err,
                    });
                    continue;
                }
            };
            let timestamps = episode_data
                .feature_files
                .get(key)
                .and_then(|f| Option::zip(f.from_timestamp, f.to_timestamp));
            // Invalid `video.fps` was cleared (with a warning) when the metadata was parsed.
            let fps = feature
                .info
                .as_ref()
                .and_then(|feature_info| feature_info.video_fps)
                .unwrap_or_else(|| f64::from(info.fps));
            match resolve_video(file, timestamps, fps) {
                Ok(source) => {
                    videos.insert(key.clone(), source);
                }
                Err(err) => diagnostics.add(LeRobotDiagnostic::FailedFeature {
                    episode: episode_data.episode_index,
                    feature: Some(key.clone()),
                    err,
                }),
            }
        }
    }

    Ok(EpisodeAddress {
        data_file,
        rows: Some(rows),
        videos,
    })
}

/// Resolve one episode's slice of a (possibly shared) video file.
///
/// `timestamps` is the episode's half-open `[from, to)` second range within the file, as
/// recorded in `meta/episodes`; `None` streams the whole file. A degenerate (zero-length)
/// or unusable range resolves nothing.
fn resolve_video(
    file: PathBuf,
    timestamps: Option<(f64, f64)>,
    fps: f64,
) -> Result<VideoSource, LeRobotError> {
    let window = match timestamps {
        Some((from, to)) => {
            let window = Option::zip(
                std::time::Duration::try_from_secs_f64(from).ok(),
                std::time::Duration::try_from_secs_f64(to).ok(),
            )
            .and_then(|(from, to)| TimeWindow::new(from, to));
            let Some(window) = window else {
                return Err(LeRobotError::InvalidDatasetInfo(format!(
                    "empty or unusable video time range {from}..{to}\nFile path: {}",
                    file.display()
                )));
            };
            Some(window)
        }
        None => None,
    };
    Ok(VideoSource::Stream { file, window, fps })
}

/// Metadata for a v3 `LeRobot` dataset, as read from the files in its `meta` directory.
struct LeRobotDatasetV3Metadata {
    info: LeRobotDatasetV3Info,
    tasks: HashMap<TaskIndex, String>,
    subtasks: HashMap<SubtaskIndex, String>,
    episodes: BTreeMap<EpisodeIndex, LeRobotEpisodeV3MetaData>,
}

impl LeRobotDatasetV3Metadata {
    /// Loads all metadata files from the provided `meta/` directory.
    fn load_from_directory(
        metadir: impl AsRef<Path>,
        diagnostics: &mut LeRobotDiagnostics,
    ) -> Result<Self, LeRobotError> {
        let metadir = metadir.as_ref();

        let episodes_metadata =
            LeRobotEpisodeV3MetaData::load_from_directory(metadir.join("episodes"), diagnostics)?;
        let info = LeRobotDatasetV3Info::load_from_json_file(metadir.join("info.json"))?;

        // Feature-scoped: a missing or corrupt task table drops its text emits (there
        // is nothing to join against), never the dataset.
        let tasks = match load_index_text_parquet(
            metadir.join("tasks.parquet"),
            "task_index",
            "task",
        ) {
            Ok(tasks) => {
                if tasks.len() != info.total_tasks {
                    diagnostics.add(LeRobotDiagnostic::MetadataWarning {
                        reason: format!("The dataset declares {} tasks in info.json, but tasks.parquet defines {}", info.total_tasks, tasks.len()),
                    });
                }
                tasks
                    .into_iter()
                    .map(|(index, task)| (TaskIndex(index), task))
                    .collect()
            }
            Err(err) => {
                diagnostics.add(LeRobotDiagnostic::SkippedFeature {
                    feature: FeatureKey::from("task_index".to_owned()),
                    err,
                });
                HashMap::default()
            }
        };

        let subtasks_path = metadir.join("subtasks.parquet");
        let subtasks = if subtasks_path.is_file() {
            match load_index_text_parquet(subtasks_path, "subtask_index", "subtask") {
                Ok(subtasks) => subtasks
                    .into_iter()
                    .map(|(index, subtask)| (SubtaskIndex(index), subtask))
                    .collect(),
                Err(err) => {
                    diagnostics.add(LeRobotDiagnostic::SkippedFeature {
                        feature: FeatureKey::from("subtask_index".to_owned()),
                        err,
                    });
                    HashMap::default()
                }
            }
        } else {
            HashMap::default()
        };

        // Key episodes by their own index; the ordered map makes every iteration ascending,
        // which the importer relies on when announcing one recording per episode.
        let episodes = episodes_metadata
            .into_iter()
            .map(|ep| (ep.episode_index, ep))
            .collect();

        Ok(Self {
            info,
            tasks,
            subtasks,
            episodes,
        })
    }
}

/// The name pandas gives a dataframe's unnamed index column when writing parquet.
///
/// `LeRobot` task tables hold the task text in the dataframe index: a named index is
/// stored under its own name (`task`, `subtask`), an unnamed one under this name.
/// See <https://pandas.pydata.org/docs/development/developer.html#storing-pandas-dataframe-objects-in-apache-parquet-format>.
const PANDAS_UNNAMED_INDEX_COLUMN: &str = "__index_level_0__";

/// Load a `(index, text)` lookup table (tasks/subtasks) from a parquet file.
///
/// The text is read from `text_column`, falling back to the pandas unnamed-index column.
fn load_index_text_parquet(
    filepath: impl AsRef<Path>,
    index_column: &str,
    text_column: &str,
) -> Result<HashMap<usize, String>, LeRobotError> {
    let filepath = filepath.as_ref();
    let parquet_data = File::open(filepath).map_err(|err| LeRobotError::io(err, filepath))?;

    let reader = ParquetRecordBatchReaderBuilder::try_new(parquet_data)
        .map_err(|err| LeRobotError::metadata_read(err, filepath))?
        .build()
        .map_err(|err| LeRobotError::metadata_read(err, filepath))?;

    let mut entries = HashMap::default();
    for record_batch in reader {
        let batch = record_batch.map_err(|err| {
            LeRobotError::InvalidDatasetInfo(format!(
                "failed to read the task table: {err}. File path: {}",
                filepath.display()
            ))
        })?;

        // A column of the wrong type would otherwise yield a silently empty table, and
        // every index would resolve to nothing with no error to explain why.
        let column = |name: &str| {
            batch.column_by_name(name).ok_or_else(|| {
                LeRobotError::InvalidDatasetInfo(format!(
                    "the task table is missing its `{name}` column. File path: {}",
                    filepath.display()
                ))
            })
        };
        let indices = column(index_column)?
            .downcast_array_ref::<Int64Array>()
            .ok_or_else(|| {
                LeRobotError::InvalidDatasetInfo(format!(
                    "the task table's `{index_column}` column is not an `Int64`. File path: {}",
                    filepath.display()
                ))
            })?;
        let (text_name, text_array) = [text_column, PANDAS_UNNAMED_INDEX_COLUMN]
            .into_iter()
            .find_map(|name| batch.column_by_name(name).map(|column| (name, column)))
            .ok_or_else(|| {
                LeRobotError::InvalidDatasetInfo(format!(
                    "the task table is missing its `{text_column}` column (and the \
                     `{PANDAS_UNNAMED_INDEX_COLUMN}` fallback). File path: {}",
                    filepath.display()
                ))
            })?;
        let texts = task_texts(text_array, text_name, filepath)?;

        for (index, text) in std::iter::zip(indices, &texts) {
            let (Some(index), Some(text)) = (index, text) else {
                continue;
            };
            // A negative index must not wrap into a huge key; drop the row.
            if let Ok(index) = usize::try_from(index) {
                entries.insert(index, text.to_owned());
            }
        }
    }

    Ok(entries)
}

/// Read a task table's text column in any Arrow string encoding. A non-string column is
/// rejected loudly rather than stringified.
///
/// The subtasks table is read through the same path.
fn task_texts(
    array: &arrow::array::ArrayRef,
    column: &str,
    filepath: &Path,
) -> Result<StringArray, LeRobotError> {
    normalize_string_array(array).map_err(|err| {
        LeRobotError::InvalidDatasetInfo(format!(
            "failed to read task text from `{column}`: {err}. File path: {}",
            filepath.display()
        ))
    })
}

/// File metadata for a specific feature (video or image) in a `LeRobot` dataset.
///
/// In v3 datasets, each video/image feature can have its own chunk and file indices,
/// allowing multiple episodes to share the same video file efficiently.
#[derive(Debug, Clone)]
struct FeatureV3FileMetadata {
    /// Chunk index where the feature's file is located
    chunk_index: usize,

    /// File index within the chunk
    file_index: usize,

    /// Start of the episode's slice of the file, in seconds since the start of the file
    from_timestamp: Option<f64>,

    /// Exclusive end of the episode's slice, in seconds since the start of the file
    to_timestamp: Option<f64>,
}

/// Episode metadata for a `LeRobot` v3 dataset.
///
/// Contains file location information for both the episode data and individual video/image features.
#[derive(Debug, Clone)]
struct LeRobotEpisodeV3MetaData {
    /// The index of this episode
    episode_index: EpisodeIndex,

    /// Chunk index for the episode's main data file
    data_chunk_index: usize,

    /// File index within the chunk for the episode's main data
    data_file_index: usize,

    /// First row of this episode, as a row index over the whole dataset (the `index`
    /// column), not within its data file
    dataset_from_index: Option<u64>,

    /// One past the last row of this episode, in the same dataset-wide row space
    dataset_to_index: Option<u64>,

    /// File metadata for video/image features, keyed by feature name (e.g., `observation.images.cam_high`)
    feature_files: HashMap<FeatureKey, FeatureV3FileMetadata>,
}

impl LeRobotEpisodeV3MetaData {
    fn load_from_directory(
        metadir: impl AsRef<Path>,
        diagnostics: &mut LeRobotDiagnostics,
    ) -> Result<Vec<Self>, LeRobotError> {
        // Walk all subdirectories and load episode data files.
        let metadir = metadir.as_ref();
        let mut all_episodes = vec![];
        for entry in std::fs::read_dir(metadir).map_err(|err| LeRobotError::io(err, metadir))? {
            let entry = entry.map_err(|err| LeRobotError::io(err, metadir))?;
            let path = entry.path();
            let path = path.as_path();

            re_log::trace!("Loading episode metadata from: {path:?}");

            if path.is_dir() {
                for chunk_entry in
                    std::fs::read_dir(path).map_err(|err| LeRobotError::io(err, path))?
                {
                    let chunk_entry = chunk_entry.map_err(|err| LeRobotError::io(err, path))?;
                    let chunk_path = chunk_entry.path();

                    // Only the parquet files are ours; a `.DS_Store` or a stray README
                    // must not fail the dataset.
                    if chunk_path.is_file()
                        && chunk_path
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("parquet"))
                    {
                        let chunk_parquet = ParquetRecordBatchReaderBuilder::try_new(
                            File::open(&chunk_path)
                                .map_err(|err| LeRobotError::io(err, chunk_path.clone()))?,
                        )
                        .map_err(|err| LeRobotError::metadata_read(err, chunk_path.clone()))?
                        .build()
                        .map_err(|err| LeRobotError::metadata_read(err, chunk_path.clone()))?;

                        // These parquet files hold episode metadata, not episode data:
                        // one row per episode, with columns that point at where the
                        // actual data and video files live (chunk index, file index,
                        // row range, timestamps).
                        let episodes_metadata: Vec<_> = chunk_parquet
                            .filter_map(|batch| {
                                let batch = batch.ok()?;

                                let episode_index = batch
                                    .column_by_name("episode_index")?
                                    .as_any()
                                    .downcast_ref::<Int64Array>()?;

                                let data_chunk_index = batch
                                    .column_by_name("data/chunk_index")?
                                    .as_any()
                                    .downcast_ref::<Int64Array>()?;

                                let data_file_index = batch
                                    .column_by_name("data/file_index")?
                                    .as_any()
                                    .downcast_ref::<Int64Array>()?;

                                Some(Self::collect_episode_metadata(
                                    &batch,
                                    episode_index,
                                    data_chunk_index,
                                    data_file_index,
                                    diagnostics,
                                ))
                            })
                            .flatten()
                            .collect();

                        all_episodes.extend(episodes_metadata);
                    }
                }
            }
        }

        Ok(all_episodes)
    }

    fn collect_episode_metadata(
        batch: &RecordBatch,
        episode_index: &Int64Array,
        data_chunk_index: &Int64Array,
        data_file_index: &Int64Array,
        diagnostics: &mut LeRobotDiagnostics,
    ) -> Vec<Self> {
        // Column pattern: "videos/{feature_name}/{field}" where field is chunk_index,
        // file_index, from_timestamp, to_timestamp.
        let feature_metadata = Self::parse_feature_metadata(batch);

        let mut episodes = Vec::with_capacity(batch.num_rows());
        for i in 0..batch.num_rows() {
            // Build feature_files map for this episode
            let feature_files = feature_metadata
                .iter()
                .filter_map(|(feature_name, metadata)| {
                    // Only include if both chunk_index and file_index are present
                    let chunk_index = metadata.chunk_index.as_ref()?;
                    let file_index = metadata.file_index.as_ref()?;

                    // A negative index must not wrap into a huge index and with it a
                    // video path that cannot exist; drop the feature's file instead.
                    let chunk_index = usize::try_from(chunk_index.value(i)).ok()?;
                    let file_index = usize::try_from(file_index.value(i)).ok()?;

                    Some((
                        FeatureKey::from(feature_name.to_string()),
                        FeatureV3FileMetadata {
                            chunk_index,
                            file_index,
                            from_timestamp: metadata.from_timestamp.as_ref().and_then(
                                |timestamps| timestamps.is_valid(i).then(|| timestamps.value(i)),
                            ),
                            to_timestamp: metadata.to_timestamp.as_ref().and_then(|timestamps| {
                                timestamps.is_valid(i).then(|| timestamps.value(i))
                            }),
                        },
                    ))
                })
                .collect();

            let row_index_at = |column: &str| {
                let c = batch
                    .column_by_name(column)
                    .and_then(|c| c.downcast_array_ref::<Int64Array>())
                    .filter(|c| c.is_valid(i))?;
                u64::try_from(c.value(i)).ok()
            };

            // Episode-scoped: a row whose indices are negative is dropped, not wrapped
            // into a huge index.
            let indices = Option::zip(
                usize::try_from(episode_index.value(i)).ok(),
                Option::zip(
                    usize::try_from(data_chunk_index.value(i)).ok(),
                    usize::try_from(data_file_index.value(i)).ok(),
                ),
            );
            let Some((episode_index, (data_chunk_index, data_file_index))) = indices else {
                diagnostics.add(LeRobotDiagnostic::SkippedMetadataRow {
                    episode_index: episode_index.value(i),
                });
                continue;
            };

            episodes.push(Self {
                episode_index: EpisodeIndex(episode_index),
                data_chunk_index,
                data_file_index,
                dataset_from_index: row_index_at("dataset_from_index"),
                dataset_to_index: row_index_at("dataset_to_index"),
                feature_files,
            });
        }
        episodes
    }

    /// Parse feature-specific file metadata from a [`RecordBatch`].
    ///
    /// Looks for columns matching pattern `videos/{feature_name}/{field}`
    /// and groups them by feature name.
    fn parse_feature_metadata(batch: &RecordBatch) -> HashMap<Arc<str>, FeatureMetadataColumns> {
        let mut features: HashMap<Arc<str>, FeatureMetadataColumns> = HashMap::default();
        let schema = batch.schema();

        for field in schema.fields() {
            let column_name = field.name();

            // Look for columns like "videos/{feature_name}/chunk_index"
            if let Some(rest) = column_name.strip_prefix("videos/")
                && let Some((feature_name, field_name)) = rest.rsplit_once('/')
            {
                let entry = features.entry(Arc::from(feature_name)).or_default();

                match field_name {
                    "chunk_index" => {
                        if let Some(col) = batch
                            .column_by_name(column_name)
                            .and_then(|c| c.downcast_array_ref::<Int64Array>())
                        {
                            entry.chunk_index = Some(col.clone());
                        }
                    }
                    "file_index" => {
                        if let Some(col) = batch
                            .column_by_name(column_name)
                            .and_then(|c| c.downcast_array_ref::<Int64Array>())
                        {
                            entry.file_index = Some(col.clone());
                        }
                    }
                    "from_timestamp" => {
                        // Datasets vary between `f32` and `f64` here; cast before
                        // downcasting.
                        if let Some(col) = timestamps_as_f64(batch.column_by_name(column_name)) {
                            entry.from_timestamp = Some(col);
                        }
                    }
                    "to_timestamp" => {
                        if let Some(col) = timestamps_as_f64(batch.column_by_name(column_name)) {
                            entry.to_timestamp = Some(col);
                        }
                    }
                    _ => {} // Ignore unknown fields
                }
            }
        }

        features
    }
}

/// Structure to hold Arrow arrays for feature metadata during parsing.
#[derive(Default)]
struct FeatureMetadataColumns {
    chunk_index: Option<Int64Array>,
    file_index: Option<Int64Array>,
    from_timestamp: Option<Float64Array>,
    to_timestamp: Option<Float64Array>,
}

/// `LeRobot` dataset metadata, from `meta/info.json`.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LeRobotDatasetV3Info {
    /// The type of the robot.
    robot_type: Option<String>,

    /// The version of the `LeRobot` codebase the dataset was created for.
    codebase_version: String,

    /// The total number of unique episodes in the dataset.
    ///
    /// Informational only — the files on disk are the truth, so its absence never
    /// gates the parse.
    #[serde(default)]
    total_episodes: usize,

    /// The total number of unique frames in the dataset.
    ///
    /// Informational only, like [`Self::total_episodes`].
    #[serde(default)]
    total_frames: usize,

    /// The total number of unique tasks in the dataset.
    ///
    /// Informational only, like [`Self::total_episodes`].
    #[serde(default)]
    total_tasks: usize,

    /// The amount of episodes per chunk.
    ///
    /// Unused in v3 (file locations come from the per-episode chunk and file index
    /// columns), so informational only, like [`Self::total_episodes`].
    #[serde(default)]
    chunks_size: usize,

    /// The path template for accessing episode data files.
    data_path: String,

    /// The path template for accessing video files for an episode.
    video_path: Option<String>,

    /// The frame rate of the recorded episode data.
    fps: f32,

    /// A mapping of feature names to their respective [`Feature`] definitions,
    /// ordered so the emitted chunks come out in a deterministic order.
    features: BTreeMap<FeatureKey, Feature>,
}

impl LeRobotDatasetV3Info {
    /// Loads `LeRobotDatasetInfo` from a JSON file.
    ///
    /// The `LeRobot` dataset info file is typically stored under `meta/info.json`.
    fn load_from_json_file(filepath: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let info_file = File::open(filepath.as_ref())
            .map_err(|err| LeRobotError::io(err, filepath.as_ref()))?;
        let reader = BufReader::new(info_file);

        serde_json::from_reader(reader).map_err(|err| LeRobotError::json(err, filepath.as_ref()))
    }

    /// Retrieve the metadata for a specific feature.
    fn feature(&self, feature_key: &str) -> Option<&Feature> {
        self.features.get(feature_key)
    }

    /// Generates the file path for a given episode's Parquet data.
    fn episode_data_path(&self, episode_data: &LeRobotEpisodeV3MetaData) -> PathBuf {
        // TODO(gijsd): Need a better way to handle this, as this only supports the default.
        self.data_path
            .replace(
                "{chunk_index:03d}",
                &format!("{:03}", episode_data.data_chunk_index),
            )
            .replace(
                "{file_index:03d}",
                &format!("{:03}", episode_data.data_file_index),
            )
            .into()
    }

    /// Generates the file path for a video observation of a given episode.
    ///
    /// In v3 datasets, video files are organized by feature-specific chunk and file indices,
    /// which are stored in the episode metadata and may differ from the episode data indices.
    fn video_path(
        &self,
        feature_key: &FeatureKey,
        episode_data: &LeRobotEpisodeV3MetaData,
    ) -> Result<PathBuf, LeRobotError> {
        let feature = self
            .feature(feature_key.as_str())
            .ok_or_else(|| LeRobotError::InvalidFeatureKey(feature_key.clone()))?;

        if feature.dtype != DType::Video {
            return Err(LeRobotError::InvalidFeatureDtype {
                key: feature_key.clone(),
                expected: DType::Video,
                actual: feature.dtype,
            });
        }

        let video_path_template = self
            .video_path
            .as_ref()
            .ok_or_else(|| LeRobotError::MissingDatasetInfo("video_path".to_owned()))?;

        if let Some(file_metadata) = episode_data.feature_files.get(feature_key) {
            // Use feature-specific chunk and file indices (v3 format)
            Ok(video_path_template
                .replace("{video_key}", feature_key.as_str())
                .replace(
                    "{chunk_index:03d}",
                    &format!("{:03}", file_metadata.chunk_index),
                )
                .replace(
                    "{file_index:03d}",
                    &format!("{:03}", file_metadata.file_index),
                )
                .into())
        } else {
            // No per-feature file metadata: the template addresses the file through
            // episode-based placeholders.
            Ok(video_path_template
                .replace(
                    "{episode_chunk:03d}",
                    &format!("{:03}", episode_data.data_chunk_index),
                )
                .replace(
                    "{episode_index:06d}",
                    &format!("{:06}", episode_data.episode_index.0),
                )
                .replace("{video_key}", feature_key.as_str())
                .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::LeRobotDataset;

    #[test]
    fn skipped_episodes_warn_once_on_success_and_failure() {
        for has_valid_episode in [true, false] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let mut info = default_info(&scalar_features());
            info["total_tasks"] = serde_json::json!(1);
            write_info(root, &info);
            write_episodes_meta(
                root,
                &[
                    row(0, has_valid_episode.then_some(0), Some(10)),
                    row(1, None, Some(10)),
                    row(2, Some(10), Some(30)),
                ],
            );
            write_tasks(root);
            write_data(root, &[10], false);

            let mut diagnostics = LeRobotDiagnostics::default();
            let result = parse(root, &mut diagnostics);
            if has_valid_episode {
                let data = result.unwrap();
                assert_eq!(
                    data.episodes.keys().copied().collect::<Vec<_>>(),
                    vec![EpisodeIndex(0)]
                );
                assert!(matches!(
                    diagnostics.entries(),
                    [
                        LeRobotDiagnostic::SkippedEpisode {
                            episode: EpisodeIndex(1),
                            ..
                        },
                        LeRobotDiagnostic::SkippedEpisode {
                            episode: EpisodeIndex(2),
                            ..
                        },
                    ]
                ));
            } else {
                assert!(matches!(result, Err(LeRobotError::NoEpisodes { .. })));
            }

            let warnings = diagnostics.summary_messages();
            assert_eq!(warnings.len(), 1);
            let count = if has_valid_episode { 2 } else { 3 };
            assert!(warnings[0].starts_with(&format!("Skipping episodes ({count}x)")));
            assert!(warnings[0].contains("\n- episode 1:"));
            assert!(warnings[0].contains("\n- episode 2:"));
        }
    }

    use super::*;

    use arrow::array::{ArrayRef, BinaryArray, Float32Array, RecordBatchOptions, StructArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use parquet::arrow::ArrowWriter;

    use crate::LeRobotConfig;

    /// Build a single `RecordBatch` from fields and columns, using the metadata/options-aware
    /// constructors our clippy config mandates.
    fn test_batch(fields: Vec<Field>, columns: Vec<arrow::array::ArrayRef>) -> RecordBatch {
        let schema = Schema::new_with_metadata(fields, Default::default());
        RecordBatch::try_new_with_options(Arc::new(schema), columns, &RecordBatchOptions::default())
            .unwrap()
    }

    fn synthetic_episode(index: usize) -> LeRobotEpisodeV3MetaData {
        LeRobotEpisodeV3MetaData {
            episode_index: EpisodeIndex(index),
            data_chunk_index: 0,
            data_file_index: 0,
            dataset_from_index: None,
            dataset_to_index: None,
            feature_files: HashMap::default(),
        }
    }

    /// Datasets vary between `f32` and `f64` for the per-episode video timestamps; both
    /// produce a window.
    #[test]
    fn f32_video_timestamps_still_produce_windows() {
        let fields = vec![
            Field::new("episode_index", DataType::Int64, false),
            Field::new("data/chunk_index", DataType::Int64, false),
            Field::new("data/file_index", DataType::Int64, false),
            Field::new("videos/cam/chunk_index", DataType::Int64, false),
            Field::new("videos/cam/file_index", DataType::Int64, false),
            Field::new("videos/cam/from_timestamp", DataType::Float32, false),
            Field::new("videos/cam/to_timestamp", DataType::Float32, false),
        ];
        let batch = test_batch(
            fields,
            vec![
                Arc::new(Int64Array::from(vec![0_i64])),
                Arc::new(Int64Array::from(vec![0_i64])),
                Arc::new(Int64Array::from(vec![0_i64])),
                Arc::new(Int64Array::from(vec![0_i64])),
                Arc::new(Int64Array::from(vec![0_i64])),
                Arc::new(Float32Array::from(vec![1.5_f32])),
                Arc::new(Float32Array::from(vec![3.0_f32])),
            ],
        );

        let episodes = LeRobotEpisodeV3MetaData::collect_episode_metadata(
            &batch,
            batch.column(0).as_any().downcast_ref().unwrap(),
            batch.column(1).as_any().downcast_ref().unwrap(),
            batch.column(2).as_any().downcast_ref().unwrap(),
            &mut LeRobotDiagnostics::default(),
        );
        let file = episodes[0].feature_files.get("cam").expect("cam parsed");
        assert_eq!(file.from_timestamp, Some(1.5));
        assert_eq!(file.to_timestamp, Some(3.0));
    }

    #[test]
    fn invalid_video_window_is_collected_without_dropping_episode() {
        let key = FeatureKey::from("camera".to_owned());
        let mut features = scalar_features();
        features["camera"] =
            serde_json::json!({ "dtype": "video", "shape": [3, 4, 4], "names": null });
        let mut json = default_info(&features);
        json["video_path"] = "videos/camera.mp4".into();
        let info: LeRobotDatasetV3Info = serde_json::from_value(json).unwrap();
        let mut episode = synthetic_episode(7);
        episode.dataset_from_index = Some(0);
        episode.dataset_to_index = Some(10);
        episode.feature_files.insert(
            key.clone(),
            FeatureV3FileMetadata {
                chunk_index: 0,
                file_index: 0,
                from_timestamp: Some(2.0),
                to_timestamp: Some(1.0),
            },
        );
        let mut diagnostics = LeRobotDiagnostics::default();
        let address = resolve_episode(
            Path::new("dataset"),
            &info,
            &episode,
            &[(&key, &info.features[&key])],
            &HashMap::default(),
            &mut diagnostics,
        )
        .unwrap();
        assert!(address.videos.is_empty());
        assert!(
            matches!(diagnostics.entries(), [LeRobotDiagnostic::FailedFeature {
            episode: EpisodeIndex(7), feature: Some(feature), err: LeRobotError::InvalidDatasetInfo(reason),
        }] if feature == &key && reason.contains("2..1") && reason.contains("videos/camera.mp4"))
        );
    }

    /// A camera's own `video.fps` wins over the dataset fps when present.
    #[test]
    fn feature_info_carries_the_camera_fps() {
        let json = r#"{
            "dtype": "video", "shape": [3, 240, 320], "names": null,
            "info": { "video.fps": 15.0, "video.codec": "h264" }
        }"#;
        let feature: Feature = serde_json::from_str(json).unwrap();
        assert_eq!(feature.info.unwrap().video_fps, Some(15.0));

        let json = r#"{ "dtype": "video", "shape": [3, 240, 320], "names": null }"#;
        let feature: Feature = serde_json::from_str(json).unwrap();
        assert!(feature.info.is_none());
    }

    /// A degenerate or unusable timestamp range resolves no video; a valid range maps
    /// directly onto a half-open [`TimeWindow`]; absent timestamps stream the whole file.
    #[test]
    fn video_windows_map_the_episode_range() {
        let file = || PathBuf::from("videos/observation.image/chunk-000/file-000.mp4");

        assert!(resolve_video(file(), Some((1.0, 1.0)), 30.0).is_err());
        assert!(resolve_video(file(), Some((2.0, 1.0)), 30.0).is_err());
        assert!(resolve_video(file(), Some((-1.0, 1.0)), 30.0).is_err());

        match resolve_video(file(), Some((1.0, 2.0)), 30.0) {
            Ok(VideoSource::Stream {
                window: Some(window),
                ..
            }) => {
                assert_eq!(window.start(), std::time::Duration::from_secs(1));
                assert_eq!(window.end(), std::time::Duration::from_secs(2));
            }
            _ => panic!("a valid range must resolve a windowed stream"),
        }

        assert!(matches!(
            resolve_video(file(), None, 30.0),
            Ok(VideoSource::Stream { window: None, .. })
        ));
    }

    fn v3_fixture() -> PathBuf {
        std::env::var_os("CARGO_MANIFEST_DIR")
            .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
            .join("../re_importer/tests/assets/lerobot/v30_apple_storage")
    }

    /// Episode addresses carry the `dataset_from_index..dataset_to_index` ranges from
    /// `meta/episodes`: exclusive ends, no overlap, and their union covers the shared
    /// file exactly.
    #[test]
    fn v3_fixture_resolves_row_ranges_from_metadata() {
        let dataset = LeRobotDataset::open(v3_fixture()).expect("fixture parses");

        let ranges: Vec<_> = dataset
            .episode_addresses()
            .map(|address| address.rows.expect("v3 addresses carry a row range"))
            .collect();

        assert_eq!(
            ranges,
            vec![
                re_span::Span::from_start_end(0, 299),
                re_span::Span::from_start_end(299, 599),
                re_span::Span::from_start_end(599, 899),
            ]
        );
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end(), pair[1].start, "no overlap, no gap");
        }
    }

    /// Consecutive episodes' windows are contiguous half-open intervals over the shared
    /// file: each `to_timestamp` is exclusive and equals the next episode's `from`, so
    /// every boundary frame lands in exactly one episode.
    #[test]
    fn v3_fixture_video_windows_are_contiguous() {
        let dataset = LeRobotDataset::open(v3_fixture()).expect("fixture parses");

        let windows: Vec<_> = dataset
            .episode_addresses()
            .map(|address| {
                address
                    .videos
                    .values()
                    .find_map(|source| match source {
                        VideoSource::Stream { window, .. } => *window,
                        VideoSource::Asset { .. } => None,
                    })
                    .expect("fixture episodes resolve a windowed video")
            })
            .collect();

        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].start(), std::time::Duration::ZERO);
        assert_eq!(windows[0].end(), windows[1].start());
        assert_eq!(windows[1].end(), windows[2].start());
    }

    #[test]
    fn episode_indices_iterate_in_ascending_order() {
        // The importer relies on ascending iteration order regardless of insertion order.
        // Repeated with new maps: a `HashMap`-backed regression only passes a run by luck.
        let scrambled = [4, 0, 3, 1, 2];
        for _ in 0..10 {
            let metadata = LeRobotDatasetV3Metadata {
                info: LeRobotDatasetV3Info {
                    robot_type: None,
                    codebase_version: "v3.0".to_owned(),
                    total_episodes: scrambled.len(),
                    total_frames: 0,
                    total_tasks: 0,
                    chunks_size: 1000,
                    data_path: String::new(),
                    video_path: None,
                    fps: 30.0,
                    features: BTreeMap::new(),
                },
                tasks: HashMap::default(),
                subtasks: HashMap::default(),
                episodes: scrambled
                    .into_iter()
                    .map(|index| (EpisodeIndex(index), synthetic_episode(index)))
                    .collect(),
            };

            assert_eq!(
                metadata.episodes.keys().copied().collect::<Vec<_>>(),
                (0..scrambled.len()).map(EpisodeIndex).collect::<Vec<_>>()
            );
        }
    }

    // -----------------------------------------------------------------------
    // Skip-scope tests: a defect only takes down what it actually touches.
    // Every test asserts on `episodes()` and stream output, never on log text.

    /// One episode's row in `meta/episodes`.
    struct EpisodeRow {
        index: i64,
        file_index: i64,
        from: Option<i64>,
        to: Option<i64>,
    }

    fn row(index: i64, from: Option<i64>, to: Option<i64>) -> EpisodeRow {
        EpisodeRow {
            index,
            file_index: 0,
            from,
            to,
        }
    }

    fn write_parquet(path: &Path, fields: Vec<Field>, columns: Vec<ArrayRef>) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let batch = test_batch(fields, columns);
        let mut writer =
            ArrowWriter::try_new(std::fs::File::create(path).unwrap(), batch.schema(), None)
                .unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
    }

    /// The scalar half every fixture shares: one float feature plus the bookkeeping columns.
    fn scalar_features() -> serde_json::Value {
        serde_json::json!({
            "observation.state": { "dtype": "float32", "shape": [1], "names": null },
            "task_index": { "dtype": "int64", "shape": [1], "names": null },
            "frame_index": { "dtype": "int64", "shape": [1], "names": null },
            "timestamp": { "dtype": "float32", "shape": [1], "names": null },
            "episode_index": { "dtype": "int64", "shape": [1], "names": null },
            "index": { "dtype": "int64", "shape": [1], "names": null },
        })
    }

    // The `{chunk_index:03d}` placeholders are LeRobot's own template syntax, not Rust formatting.
    #[expect(clippy::literal_string_with_formatting_args)]
    fn default_info(features: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "codebase_version": "v3.0",
            "robot_type": null,
            "total_episodes": 0,
            "total_frames": 0,
            "total_tasks": 0,
            "chunks_size": 1000,
            "fps": 30.0,
            "data_path": "data/chunk-{chunk_index:03d}/file-{file_index:03d}.parquet",
            "features": features,
        })
    }

    fn write_info(root: &Path, info: &serde_json::Value) {
        let meta = root.join("meta");
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(meta.join("info.json"), serde_json::to_string(info).unwrap()).unwrap();
    }

    fn write_episodes_meta(root: &Path, rows: &[EpisodeRow]) {
        let n = rows.len();
        write_parquet(
            &root.join("meta/episodes/chunk-000/file-000.parquet"),
            vec![
                Field::new("episode_index", DataType::Int64, false),
                Field::new("data/chunk_index", DataType::Int64, false),
                Field::new("data/file_index", DataType::Int64, false),
                Field::new("dataset_from_index", DataType::Int64, true),
                Field::new("dataset_to_index", DataType::Int64, true),
            ],
            vec![
                Arc::new(Int64Array::from(
                    rows.iter().map(|r| r.index).collect::<Vec<_>>(),
                )),
                Arc::new(Int64Array::from(vec![0_i64; n])),
                Arc::new(Int64Array::from(
                    rows.iter().map(|r| r.file_index).collect::<Vec<_>>(),
                )),
                Arc::new(Int64Array::from(
                    rows.iter().map(|r| r.from).collect::<Vec<_>>(),
                )),
                Arc::new(Int64Array::from(
                    rows.iter().map(|r| r.to).collect::<Vec<_>>(),
                )),
            ],
        );
    }

    fn write_tasks(root: &Path) {
        write_tasks_with_column(root, PANDAS_UNNAMED_INDEX_COLUMN);
    }

    fn write_tasks_with_column(root: &Path, text_column: &str) {
        write_parquet(
            &root.join("meta/tasks.parquet"),
            vec![
                Field::new("task_index", DataType::Int64, false),
                Field::new(text_column, DataType::Utf8, false),
            ],
            vec![
                Arc::new(Int64Array::from(vec![0_i64])),
                Arc::new(StringArray::from(vec!["pick the apple"])),
            ],
        );
    }

    /// Write the shared data file: `episode_lens` episodes back to back, `frame_index`
    /// restarting at zero for each. `with_image` adds an encoded-image column (JPEG magic
    /// bytes) in the on-disk `struct<bytes>` shape.
    fn write_data(root: &Path, episode_lens: &[usize], with_image: bool) {
        write_data_file(root, 0, episode_lens, with_image);
    }

    fn write_data_file(root: &Path, file_index: usize, episode_lens: &[usize], with_image: bool) {
        let frame_index: Vec<i64> = episode_lens
            .iter()
            .flat_map(|len| 0..i64::try_from(*len).unwrap())
            .collect();
        let num_rows = frame_index.len();

        let mut fields = vec![
            Field::new("frame_index", DataType::Int64, false),
            Field::new("timestamp", DataType::Float64, false),
            Field::new("observation.state", DataType::Float64, false),
            Field::new("task_index", DataType::Int64, false),
        ];
        let mut columns: Vec<ArrayRef> = vec![
            Arc::new(Int64Array::from(frame_index.clone())),
            Arc::new(Float64Array::from(
                frame_index
                    .iter()
                    .map(|f| *f as f64 / 30.0)
                    .collect::<Vec<_>>(),
            )),
            Arc::new(Float64Array::from(
                (0..num_rows).map(|row| row as f64).collect::<Vec<_>>(),
            )),
            Arc::new(Int64Array::from(vec![0_i64; num_rows])),
        ];

        if with_image {
            let frames: Vec<Vec<u8>> = (0..num_rows)
                .map(|row| vec![0xFF, 0xD8, 0xFF, 0xE0, row as u8])
                .collect();
            let bytes = BinaryArray::from_iter_values(frames.iter().map(Vec::as_slice));
            let image = StructArray::from(vec![(
                Arc::new(Field::new("bytes", DataType::Binary, false)),
                Arc::new(bytes) as ArrayRef,
            )]);
            fields.push(Field::new(
                "observation.image",
                image.data_type().clone(),
                false,
            ));
            columns.push(Arc::new(image));
        }

        write_parquet(
            &root.join(format!("data/chunk-000/file-{file_index:03}.parquet")),
            fields,
            columns,
        );
    }

    /// Every chunk of one episode's stream as `(entity_path, num_rows)`, under the default config.
    fn stream_entities(dataset: &LeRobotDataset, episode: EpisodeIndex) -> Vec<(String, usize)> {
        dataset
            .stream(episode, &LeRobotConfig::default())
            .expect("every listed episode must stream")
            .map(|chunk| chunk.expect("every fixture chunk must build"))
            .map(|chunk| (chunk.entity_path().to_string(), chunk.num_rows()))
            .collect()
    }

    fn rows_of<'a>(entities: &'a [(String, usize)], entity: &str) -> Vec<&'a (String, usize)> {
        entities.iter().filter(|(path, _)| path == entity).collect()
    }

    // -----------------------------------------------------------------------
    // Episode-scoped

    #[test]
    fn open_captures_recoverable_failures_by_scope() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut features = scalar_features();
        features["observation.image"] = serde_json::json!({
            "dtype": "video", "shape": [3, 4, 4], "names": null,
            "info": { "video.fps": -1.0 }
        });
        features["observation.velocity"] = serde_json::json!({
            "dtype": "float32", "shape": [1], "names": null
        });
        write_info(root, &default_info(&features));
        write_episodes_meta(
            root,
            &[
                row(0, Some(0), Some(10)),
                row(1, None, None),
                row(-1, Some(0), Some(10)),
                EpisodeRow {
                    index: 2,
                    file_index: 1,
                    from: Some(0),
                    to: Some(10),
                },
                EpisodeRow {
                    index: 3,
                    file_index: 1,
                    from: Some(10),
                    to: Some(20),
                },
            ],
        );
        write_data(root, &[10], false);

        let dataset = LeRobotDataset::open(root).unwrap();
        assert_eq!(
            dataset.episodes().collect::<Vec<_>>(),
            vec![EpisodeIndex(0)]
        );
        let entries = dataset.diagnostics().entries();
        let data_files: Vec<_> = entries
            .iter()
            .filter_map(|entry| match entry {
                LeRobotDiagnostic::SkippedDataFile { path, .. } => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(
            data_files,
            vec![&root.join("data/chunk-000/file-001.parquet")]
        );
        assert!(entries.iter().any(|entry| matches!(entry,
            LeRobotDiagnostic::SkippedDataFile { err: LeRobotError::IO { source, .. }, .. }
                if source.kind() == std::io::ErrorKind::NotFound
        )));
        assert!(entries.iter().any(|entry| matches!(entry,
            LeRobotDiagnostic::SkippedFeature { feature, err: LeRobotError::IO { path, source } }
                if feature.as_str() == "task_index" && path == &root.join("meta/tasks.parquet")
                    && source.kind() == std::io::ErrorKind::NotFound
        )));
        assert!(entries.iter().any(|entry| matches!(entry,
            LeRobotDiagnostic::SkippedFeature { feature, .. } if feature.as_str() == "observation.velocity"
        )));
        assert!(entries.iter().any(|entry| matches!(entry,
            LeRobotDiagnostic::SkippedFeature { feature, .. } if feature.as_str() == "observation.image"
        )));
        assert!(entries.iter().any(|entry| matches!(
            entry,
            LeRobotDiagnostic::SkippedEpisode {
                episode: EpisodeIndex(1),
                ..
            }
        )));
        assert!(entries.iter().any(|entry| matches!(
            entry,
            LeRobotDiagnostic::SkippedMetadataRow { episode_index: -1 }
        )));
        assert!(entries.iter().any(|entry| matches!(entry,
            LeRobotDiagnostic::MetadataWarning { reason } if reason.contains("video.fps")
        )));
        let warnings = dataset.diagnostics().summary_messages();
        assert_eq!(warnings.len(), 5);
        assert!(warnings.iter().all(|warning| warning.contains("\n- ")));
        assert!(!stream_entities(&dataset, EpisodeIndex(0)).is_empty());
    }

    /// A bad episode metadata row (no usable row range) drops only that episode.
    #[test]
    fn corrupt_episode_row_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features()));
        write_episodes_meta(
            root,
            &[
                row(0, Some(0), Some(10)),
                row(1, None, Some(20)), // corrupt: no start index
                row(2, Some(10), Some(20)),
            ],
        );
        write_tasks(root);
        write_data(root, &[10, 10], false);

        let dataset = LeRobotDataset::open(root).expect("one bad episode must not fail open");
        assert_eq!(
            dataset.episodes().collect::<Vec<_>>(),
            vec![EpisodeIndex(0), EpisodeIndex(2)]
        );

        for episode in dataset.episodes() {
            let entities = stream_entities(&dataset, episode);
            assert_eq!(
                rows_of(&entities, "/observation.state"),
                vec![&("/observation.state".to_owned(), 10)]
            );
            assert!(
                !rows_of(&entities, "/task").is_empty(),
                "task text expected"
            );
        }
    }

    /// An episode whose data file is missing is dropped; the others keep streaming.
    #[test]
    fn missing_data_file_skips_the_episode() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features()));
        write_episodes_meta(
            root,
            &[
                row(0, Some(0), Some(10)),
                EpisodeRow {
                    index: 1,
                    file_index: 1, // no file-001.parquet on disk
                    from: Some(0),
                    to: Some(10),
                },
            ],
        );
        write_tasks(root);
        write_data(root, &[10], false);

        let dataset = LeRobotDataset::open(root).expect("one missing file must not fail open");
        assert_eq!(
            dataset.episodes().collect::<Vec<_>>(),
            vec![EpisodeIndex(0)]
        );
        assert!(!stream_entities(&dataset, EpisodeIndex(0)).is_empty());
    }

    /// `dataset_from_index`/`dataset_to_index` count rows over the whole dataset, so an
    /// episode in a later data file carries a range past its own file's row count; the
    /// range must rebase to the file, not drop the episode.
    #[test]
    fn global_row_ranges_rebase_to_each_data_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features()));
        write_episodes_meta(
            root,
            &[
                row(0, Some(0), Some(10)),
                row(1, Some(10), Some(20)),
                EpisodeRow {
                    index: 2,
                    file_index: 1,
                    from: Some(20),
                    to: Some(30),
                },
                EpisodeRow {
                    index: 3,
                    file_index: 1,
                    from: Some(30),
                    to: Some(40),
                },
            ],
        );
        write_tasks(root);
        write_data_file(root, 0, &[10, 10], false);
        write_data_file(root, 1, &[10, 10], false);

        let dataset = LeRobotDataset::open(root).expect("global row ranges must rebase");
        assert_eq!(
            dataset.episodes().collect::<Vec<_>>(),
            (0..4).map(EpisodeIndex).collect::<Vec<_>>()
        );

        let ranges: Vec<_> = dataset
            .episode_addresses()
            .map(|address| address.rows.expect("v3 addresses carry a row range"))
            .collect();
        assert_eq!(
            ranges,
            vec![
                re_span::Span::from_start_end(0, 10),
                re_span::Span::from_start_end(10, 20),
                re_span::Span::from_start_end(0, 10),
                re_span::Span::from_start_end(10, 20),
            ]
        );

        for episode in dataset.episodes() {
            let entities = stream_entities(&dataset, episode);
            assert_eq!(
                rows_of(&entities, "/observation.state"),
                vec![&("/observation.state".to_owned(), 10)]
            );
        }
    }

    /// A row range that ends beyond the data file's actual rows drops only that episode.
    #[test]
    fn out_of_range_episode_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features()));
        write_episodes_meta(
            root,
            &[
                row(0, Some(0), Some(10)),
                row(1, Some(10), Some(50)), // the file only has 20 rows
            ],
        );
        write_tasks(root);
        write_data(root, &[10, 10], false);

        let dataset = LeRobotDataset::open(root).expect("one bad range must not fail open");
        assert_eq!(
            dataset.episodes().collect::<Vec<_>>(),
            vec![EpisodeIndex(0)]
        );
        assert!(!stream_entities(&dataset, EpisodeIndex(0)).is_empty());
    }

    /// An unreadable data file drops every episode in it; with a single file, `open()` fails.
    #[test]
    fn unreadable_data_file_drops_its_episodes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features()));
        write_episodes_meta(root, &[row(0, Some(0), Some(10))]);
        write_tasks(root);
        std::fs::create_dir_all(root.join("data/chunk-000")).unwrap();
        std::fs::write(root.join("data/chunk-000/file-000.parquet"), b"not parquet").unwrap();
        assert!(matches!(
            read_data_footer(&root.join("data/chunk-000/file-000.parquet")),
            Err(LeRobotError::DataFileFooter { .. })
        ));

        assert!(matches!(
            LeRobotDataset::open(root),
            Err(LeRobotError::NoEpisodes { .. })
        ));
    }

    /// A data file with neither `frame_index` nor `timestamp` has no derivable timeline:
    /// every episode in it drops.
    #[test]
    fn data_file_without_timeline_columns_drops_its_episodes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features()));
        write_episodes_meta(root, &[row(0, Some(0), Some(3))]);
        write_tasks(root);
        write_parquet(
            &root.join("data/chunk-000/file-000.parquet"),
            vec![Field::new("observation.state", DataType::Float64, false)],
            vec![Arc::new(Float64Array::from(vec![0.0, 1.0, 2.0]))],
        );

        assert!(matches!(
            LeRobotDataset::open(root),
            Err(LeRobotError::NoEpisodes { .. })
        ));
    }

    /// A `frame_index` feature whose column is absent from the data files must not pick
    /// the `frame_index` timeline: the episode falls back to `timestamp` and streams.
    #[test]
    fn frame_index_feature_without_its_column_falls_back_to_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features())); // features list `frame_index`
        write_episodes_meta(root, &[row(0, Some(0), Some(3))]);
        write_tasks(root);
        write_parquet(
            &root.join("data/chunk-000/file-000.parquet"),
            vec![
                Field::new("timestamp", DataType::Float64, false),
                Field::new("observation.state", DataType::Float64, false),
            ],
            vec![
                Arc::new(Float64Array::from(vec![0.0, 1.0 / 30.0, 2.0 / 30.0])),
                Arc::new(Float64Array::from(vec![0.0, 1.0, 2.0])),
            ],
        );

        let dataset = LeRobotDataset::open(root).expect("a valid `timestamp` timeline exists");
        let timelines: Vec<String> = dataset
            .stream(EpisodeIndex(0), &LeRobotConfig::default())
            .expect("the episode must stream on the fallback timeline")
            .map(|chunk| chunk.expect("every fixture chunk must build"))
            .flat_map(|chunk| {
                chunk
                    .timelines()
                    .keys()
                    .map(|name| name.to_string())
                    .collect::<Vec<_>>()
            })
            .collect();
        assert!(
            timelines.iter().all(|name| name == "timestamp"),
            "every chunk must sit on the `timestamp` timeline, got {timelines:?}"
        );
        assert!(!timelines.is_empty(), "the stream must produce chunks");
    }

    /// When every episode is defective there is nothing to stream: `open()` fails.
    #[test]
    fn all_episodes_corrupt_fails_open() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features()));
        write_episodes_meta(root, &[row(0, None, None), row(1, Some(5), Some(5))]);
        write_tasks(root);
        write_data(root, &[10], false);

        assert!(matches!(
            LeRobotDataset::open(root),
            Err(LeRobotError::NoEpisodes { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // Feature-scoped

    /// A missing tasks table drops the task text and nothing else.
    #[test]
    fn missing_tasks_parquet_drops_only_task_text() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features()));
        write_episodes_meta(root, &[row(0, Some(0), Some(10))]);
        // no tasks.parquet
        write_data(root, &[10], false);

        let dataset = LeRobotDataset::open(root).expect("a missing tasks table must not fail open");
        let entities = stream_entities(&dataset, EpisodeIndex(0));
        assert!(
            rows_of(&entities, "/task").is_empty(),
            "no task chunks expected"
        );
        assert_eq!(
            rows_of(&entities, "/observation.state"),
            vec![&("/observation.state".to_owned(), 10)]
        );
    }

    /// The task text also loads when it is stored as `LargeUtf8` or `Utf8View`, not only
    /// plain `Utf8`.
    #[test]
    fn non_utf8_string_task_columns_load_text() {
        let texts: [ArrayRef; 2] = [
            Arc::new(arrow::array::LargeStringArray::from(vec!["pick the apple"])),
            Arc::new(arrow::array::StringViewArray::from(vec!["pick the apple"])),
        ];
        for texts in texts {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            write_info(root, &default_info(&scalar_features()));
            write_episodes_meta(root, &[row(0, Some(0), Some(10))]);
            write_parquet(
                &root.join("meta/tasks.parquet"),
                vec![
                    Field::new("task_index", DataType::Int64, false),
                    Field::new("task", texts.data_type().clone(), false),
                ],
                vec![Arc::new(Int64Array::from(vec![0_i64])), texts],
            );
            write_data(root, &[10], false);

            let dataset = LeRobotDataset::open(root).expect("open must succeed");
            let mut emitted_text = Vec::new();
            for chunk in dataset
                .stream(EpisodeIndex(0), &LeRobotConfig::default())
                .unwrap()
            {
                let chunk = chunk.unwrap();
                if chunk.entity_path().to_string() != "/task" {
                    continue;
                }
                let text = chunk
                    .components()
                    .get(re_sdk_types::archetypes::TextDocument::descriptor_text().component)
                    .unwrap();
                let values = text
                    .list_array
                    .values()
                    .downcast_array_ref::<StringArray>()
                    .unwrap();
                emitted_text.extend(values.iter().map(|value| value.unwrap().to_owned()));
            }
            assert_eq!(emitted_text, vec!["pick the apple"; 10]);
        }
    }

    #[test]
    fn non_string_task_column_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.parquet");
        write_parquet(
            &path,
            vec![
                Field::new("task_index", DataType::Int64, false),
                Field::new("task", DataType::Int64, false),
            ],
            vec![
                Arc::new(Int64Array::from(vec![0_i64])),
                Arc::new(Int64Array::from(vec![42_i64])),
            ],
        );

        let err = load_index_text_parquet(&path, "task_index", "task").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("Expected a string array"), "{message}");
        assert!(message.contains("`task`"), "{message}");
        assert!(message.contains(path.to_str().unwrap()), "{message}");
    }

    /// The task text also loads from a named `task` column (pandas named-index layout).
    #[test]
    fn named_task_column_loads_text() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features()));
        write_episodes_meta(root, &[row(0, Some(0), Some(10))]);
        write_tasks_with_column(root, "task");
        write_data(root, &[10], false);

        let dataset = LeRobotDataset::open(root).expect("open must succeed");
        let entities = stream_entities(&dataset, EpisodeIndex(0));
        assert!(
            !rows_of(&entities, "/task").is_empty(),
            "task text must load from the named `task` column"
        );
    }

    /// A video feature without a `video_path` template drops the video emits and nothing else.
    #[test]
    fn missing_video_template_drops_only_video() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut features = scalar_features();
        features["observation.image"] =
            serde_json::json!({ "dtype": "video", "shape": [3, 4, 4], "names": null });
        write_info(root, &default_info(&features)); // no video_path template
        write_episodes_meta(root, &[row(0, Some(0), Some(10))]);
        write_tasks(root);
        write_data(root, &[10], false);

        let dataset =
            LeRobotDataset::open(root).expect("a missing video template must not fail open");
        let entities = stream_entities(&dataset, EpisodeIndex(0));
        assert!(
            rows_of(&entities, "/observation.image").is_empty(),
            "no video chunks expected"
        );
        assert_eq!(
            rows_of(&entities, "/observation.state"),
            vec![&("/observation.state".to_owned(), 10)]
        );
    }

    /// A feature with a dtype this crate does not know is dropped; the rest streams.
    #[test]
    fn unknown_dtype_drops_only_that_feature() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut features = scalar_features();
        features["observation.pointcloud"] =
            serde_json::json!({ "dtype": "pointcloud", "shape": [1024, 3], "names": null });
        write_info(root, &default_info(&features));
        write_episodes_meta(root, &[row(0, Some(0), Some(10))]);
        write_tasks(root);
        write_data(root, &[10], false);

        let dataset = LeRobotDataset::open(root).expect("an unknown dtype must not fail open");
        let entities = stream_entities(&dataset, EpisodeIndex(0));
        assert!(rows_of(&entities, "/observation.pointcloud").is_empty());
        assert_eq!(
            rows_of(&entities, "/observation.state"),
            vec![&("/observation.state".to_owned(), 10)]
        );
    }

    /// A feature whose column is absent from the data files, or whose on-disk type cannot
    /// feed its emit, is dropped at open: the stream carries no `Err` items for it.
    #[test]
    fn feature_contradicting_the_data_schema_is_dropped() {
        // A declared feature with no column at all.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut features = scalar_features();
        features["observation.velocity"] =
            serde_json::json!({ "dtype": "float32", "shape": [1], "names": null });
        write_info(root, &default_info(&features));
        write_episodes_meta(root, &[row(0, Some(0), Some(10))]);
        write_tasks(root);
        write_data(root, &[10], false);

        let dataset = LeRobotDataset::open(root).expect("a missing column must not fail open");
        let entities = stream_entities(&dataset, EpisodeIndex(0));
        assert!(rows_of(&entities, "/observation.velocity").is_empty());
        assert_eq!(
            rows_of(&entities, "/observation.state"),
            vec![&("/observation.state".to_owned(), 10)]
        );

        // An image-dtype feature whose column is a plain float, not `struct<bytes>`.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut features = scalar_features();
        features["observation.state"] =
            serde_json::json!({ "dtype": "image", "shape": [4, 4, 3], "names": null });
        write_info(root, &default_info(&features));
        write_episodes_meta(root, &[row(0, Some(0), Some(10))]);
        write_tasks(root);
        write_data(root, &[10], false);

        let dataset = LeRobotDataset::open(root).expect("a wrong-shape column must not fail open");
        let entities = stream_entities(&dataset, EpisodeIndex(0));
        assert!(rows_of(&entities, "/observation.state").is_empty());
        assert!(
            !rows_of(&entities, "/task").is_empty(),
            "task text expected"
        );
    }

    // -----------------------------------------------------------------------
    // Dataset-scoped

    /// Defects that make every episode meaningless fail `open()`.
    #[test]
    fn dataset_scoped_defects_fail_open() {
        let write_fixture = |root: &Path, info: &serde_json::Value| {
            write_info(root, info);
            write_episodes_meta(root, &[row(0, Some(0), Some(10))]);
            write_tasks(root);
            write_data(root, &[10], false);
        };

        let mut zero_fps = default_info(&scalar_features());
        zero_fps["fps"] = 0.0.into();
        let mut wrong_version = default_info(&scalar_features());
        wrong_version["codebase_version"] = "v9.0".into();
        let empty_features = default_info(&serde_json::json!({}));

        for (defect, info) in [
            ("zero fps", zero_fps),
            ("unrecognized codebase_version", wrong_version),
            ("empty features map", empty_features),
        ] {
            let dir = tempfile::tempdir().unwrap();
            write_fixture(dir.path(), &info);
            assert!(
                LeRobotDataset::open(dir.path()).is_err(),
                "{defect} must fail open()"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Image dtype

    /// A 3-channel image feature streams one `EncodedImage` chunk per record batch,
    /// row-aligned with the episode timeline, alongside the other features.
    #[test]
    fn image_dtype_streams_encoded_images() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut features = scalar_features();
        features["observation.image"] =
            serde_json::json!({ "dtype": "image", "shape": [4, 4, 3], "names": null });
        write_info(root, &default_info(&features));
        write_episodes_meta(root, &[row(0, Some(0), Some(20))]);
        write_tasks(root);
        write_data(root, &[20], true);

        let dataset = LeRobotDataset::open(root).expect("image fixture opens");
        let entities = stream_entities(&dataset, EpisodeIndex(0));

        assert_eq!(
            rows_of(&entities, "/observation.image"),
            vec![&("/observation.image".to_owned(), 20)]
        );
        assert_eq!(
            rows_of(&entities, "/observation.state"),
            vec![&("/observation.state".to_owned(), 20)]
        );
    }

    // -----------------------------------------------------------------------
    // String dtype

    /// A string feature streams text chunks on its own entity, row-aligned with the
    /// episode timeline, alongside the other features.
    #[test]
    fn string_dtype_streams_text() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut features = scalar_features();
        features["subtask"] = serde_json::json!({ "dtype": "string", "shape": [1], "names": null });
        write_info(root, &default_info(&features));
        write_episodes_meta(root, &[row(0, Some(0), Some(3))]);
        write_tasks(root);
        write_parquet(
            &root.join("data/chunk-000/file-000.parquet"),
            vec![
                Field::new("frame_index", DataType::Int64, false),
                Field::new("timestamp", DataType::Float64, false),
                Field::new("observation.state", DataType::Float64, false),
                Field::new("subtask", DataType::Utf8, false),
            ],
            vec![
                Arc::new(Int64Array::from(vec![0_i64, 1, 2])),
                Arc::new(Float64Array::from(vec![0.0, 1.0 / 30.0, 2.0 / 30.0])),
                Arc::new(Float64Array::from(vec![0.0, 1.0, 2.0])),
                Arc::new(StringArray::from(vec!["reach", "reach", "grasp"])),
            ],
        );

        let dataset = LeRobotDataset::open(root).expect("string fixture opens");
        let entities = stream_entities(&dataset, EpisodeIndex(0));
        assert_eq!(
            rows_of(&entities, "/subtask"),
            vec![&("/subtask".to_owned(), 3)]
        );
        assert_eq!(
            rows_of(&entities, "/observation.state"),
            vec![&("/observation.state".to_owned(), 3)]
        );
    }

    /// A string feature whose column is absent from the data files is dropped at open;
    /// the rest streams.
    #[test]
    fn missing_string_column_drops_only_that_feature() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let mut features = scalar_features();
        features["subtask"] = serde_json::json!({ "dtype": "string", "shape": [1], "names": null });
        write_info(root, &default_info(&features));
        write_episodes_meta(root, &[row(0, Some(0), Some(10))]);
        write_tasks(root);
        write_data(root, &[10], false);

        let dataset = LeRobotDataset::open(root).expect("a missing column must not fail open");
        let entities = stream_entities(&dataset, EpisodeIndex(0));
        assert!(rows_of(&entities, "/subtask").is_empty());
        assert_eq!(
            rows_of(&entities, "/observation.state"),
            vec![&("/observation.state".to_owned(), 10)]
        );
    }

    // -----------------------------------------------------------------------
    // Error timing

    /// An episode whose data file breaks after `open()` fails the `stream()` call
    /// itself: the episode's data file is opened there.
    #[test]
    fn episode_errors_fail_the_stream_call() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_info(root, &default_info(&scalar_features()));
        write_episodes_meta(root, &[row(0, Some(0), Some(20))]);
        write_tasks(root);
        write_data(root, &[20], false);

        let dataset = LeRobotDataset::open(root).expect("fixture opens");
        std::fs::write(root.join("data/chunk-000/file-000.parquet"), b"not parquet").unwrap();
        assert!(matches!(
            read_data_footer(&root.join("data/chunk-000/file-000.parquet")),
            Err(LeRobotError::DataFileFooter { .. })
        ));

        let result = dataset.stream(EpisodeIndex(0), &LeRobotConfig::default());
        assert!(
            result
                .err()
                .is_some_and(|err| matches!(err, LeRobotError::EpisodeDataRead { .. })),
            "the damage must fail stream() with the read error"
        );
    }
}
