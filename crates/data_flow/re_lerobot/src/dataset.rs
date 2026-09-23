use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ahash::HashMap;
use re_chunk::Chunk;
use re_mp4_reader::TimeWindow;
use serde::{Deserialize, Serialize};

use crate::config::LeRobotConfig;
use crate::diagnostics::LeRobotDiagnostics;
use crate::error::LeRobotError;
use crate::features::{Feature, FeatureKey};
use crate::version::LeRobotDatasetVersion;
use crate::{convert, emits, parse_v2, parse_v3};

/// Version-independent metadata and resolved episode addresses produced by the parsers.
pub struct DatasetData {
    pub version: LeRobotDatasetVersion,

    /// The dataset's feature definitions.
    ///
    /// Ordered so the emits — and with them the chunk output order — are deterministic
    /// across opens and processes.
    pub features: BTreeMap<FeatureKey, Feature>,

    /// One resolved address per episode.
    ///
    /// Ordered by index, so [`LeRobotDataset::episodes`] iterates ascending — the importer
    /// announces one recording per episode in that order.
    pub episodes: BTreeMap<EpisodeIndex, EpisodeAddress>,
    pub tasks: Tasks,

    /// The dataset's recording rate, from `info.json`; maps timestamps to frame positions.
    pub fps: f64,

    /// Whether the data files carry the episode frame timeline.
    pub has_frame_index: bool,
}

/// An opened dataset with its parsed data and open-time diagnostics.
pub struct LeRobotDataset {
    path: PathBuf,

    data: DatasetData,

    diagnostics: LeRobotDiagnostics,
}

/// Fully resolved location of one episode's rows and videos.
pub struct EpisodeAddress {
    /// The parquet data file holding the episode's rows.
    pub data_file: PathBuf,

    /// Row span within `data_file`; `None` reads the whole file (v2: one file
    /// per episode).
    pub rows: Option<re_span::Span<u64>>,

    /// Source per video feature; absence means the feature has no usable source.
    pub videos: HashMap<FeatureKey, VideoSource>,
}

/// Where a video feature's bytes come from and how they are emitted.
#[derive(Clone)]
pub enum VideoSource {
    /// v2: one file per episode, logged whole as an `AssetVideo`.
    Asset { file: PathBuf },

    /// v3: a file shared across episodes, streamed as a `VideoStream`.
    Stream {
        file: PathBuf,

        /// The episode's slice of the shared file; `None` streams the whole file.
        window: Option<TimeWindow>,

        /// Maps rebased sample timestamps onto a sequence timeline.
        fps: f64,
    },
}

/// Task and subtask descriptions, joined against the `task_index`/`subtask_index` columns.
#[derive(Default)]
pub struct Tasks {
    pub tasks: HashMap<TaskIndex, String>,
    pub subtasks: HashMap<SubtaskIndex, String>,
}

impl LeRobotDataset {
    /// Open the `LeRobot` dataset at `path`, detecting its format version.
    ///
    /// Only v2 and v3 are supported here; a v1 dataset (handled separately by the importer)
    /// or an unrecognized path is an error.
    ///
    /// Opening reads metadata only (including the data files' parquet footers); no data
    /// pages are decoded until [`Self::stream`] is called.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LeRobotError> {
        let path = path.as_ref();
        let mut diagnostics = LeRobotDiagnostics::default();
        let result = match LeRobotDatasetVersion::find_version(path) {
            Some(LeRobotDatasetVersion::V2) => parse_v2::parse(path),
            Some(LeRobotDatasetVersion::V3) => parse_v3::parse(path, &mut diagnostics),
            Some(LeRobotDatasetVersion::V1) | None => Err(LeRobotError::UnsupportedVersion {
                path: path.to_path_buf(),
            }),
        };
        diagnostics.log_summaries();
        match result {
            Ok(data) => Ok(Self {
                path: path.to_path_buf(),
                data,
                diagnostics,
            }),
            Err(err) => Err(err),
        }
    }

    /// The directory this dataset was opened from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The format version detected at [`Self::open`], as a label only.
    pub fn version(&self) -> LeRobotDatasetVersion {
        self.data.version
    }

    /// Non-fatal defects collected while opening, also emitted as summary warnings.
    pub fn diagnostics(&self) -> &LeRobotDiagnostics {
        &self.diagnostics
    }

    /// Iterate the dataset's episode indices, in ascending order.
    pub fn episodes(&self) -> impl Iterator<Item = EpisodeIndex> + '_ {
        self.data.episodes.keys().copied()
    }

    /// The resolved episode addresses, in episode order. Exists only so the parser tests
    /// can assert row spans and video windows.
    #[cfg(test)]
    pub fn episode_addresses(&self) -> impl Iterator<Item = &EpisodeAddress> {
        self.data.episodes.values()
    }

    /// Stream one episode's chunks.
    pub fn stream(
        &self,
        episode: EpisodeIndex,
        config: &LeRobotConfig,
    ) -> Result<impl Iterator<Item = Result<Chunk, LeRobotError>> + use<>, LeRobotError> {
        let address = self
            .data
            .episodes
            .get(&episode)
            .ok_or(LeRobotError::InvalidEpisodeIndex(episode))?;
        let emits = emits::build_emits(
            &self.data.features,
            &address.videos,
            &self.data.tasks,
            config,
        );
        convert::execute(
            emits,
            address,
            &self.data.tasks,
            config,
            self.data.has_frame_index,
            self.data.fps,
        )
    }
}

/// Newtype wrapper for episode indices.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct EpisodeIndex(pub usize);

/// Newtype wrapper for task indices.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct TaskIndex(pub usize);

/// Newtype wrapper for subtask indices.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct SubtaskIndex(pub usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_retains_both_versions_parsed_data() {
        for (fixture, version) in [
            ("v21_apple_storage", LeRobotDatasetVersion::V2),
            ("v30_apple_storage", LeRobotDatasetVersion::V3),
        ] {
            let path = std::env::var_os("CARGO_MANIFEST_DIR")
                .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")), PathBuf::from)
                .join("../re_importer/tests/assets/lerobot")
                .join(fixture);
            let dataset = LeRobotDataset::open(&path).unwrap();
            assert_eq!(dataset.path(), path);
            assert_eq!(dataset.version(), version);
            assert_eq!(
                dataset.episodes().collect::<Vec<_>>(),
                vec![EpisodeIndex(0), EpisodeIndex(1), EpisodeIndex(2)]
            );
            assert!(dataset.diagnostics().entries().is_empty());
        }
    }
}
