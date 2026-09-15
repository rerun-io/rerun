use std::thread;

use anyhow::{Context as _, anyhow};
use crossbeam::channel::Sender;
use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_log_types::{ApplicationId, StoreId};
use re_quota_channel::send_crossbeam;

use crate::{ImportedData, Importer, ImporterError, import_file::prepare_store_info};
use re_lerobot::{
    EpisodeIndex, LeRobotConfig, LeRobotDataset, LeRobotDatasetVersion, is_lerobot_dataset,
};

/// An [`Importer`] for `LeRobot` datasets.
///
/// An example dataset which can be loaded can be found on Hugging Face: [lerobot/pusht_image](https://huggingface.co/datasets/lerobot/pusht_image)
pub struct LeRobotDatasetImporter;

impl Importer for LeRobotDatasetImporter {
    fn name(&self) -> String {
        "rerun.importers.LeRobotDataset".into()
    }

    fn import_from_path(
        &self,
        settings: &crate::ImporterSettings,
        filepath: std::path::PathBuf,
        tx: Sender<ImportedData>,
    ) -> Result<(), ImporterError> {
        if !is_lerobot_dataset(&filepath) {
            return Err(ImporterError::Incompatible(filepath));
        }

        let version = LeRobotDatasetVersion::find_version(&filepath)
            .ok_or_else(|| anyhow!("Could not determine LeRobot dataset version"))?;

        match version {
            // Handled here rather than in `LeRobotDataset::open`, which only knows v2/v3.
            LeRobotDatasetVersion::V1 => {
                re_log::error!("LeRobot 'v1.x' dataset format is unsupported.");
                Ok(())
            }
            LeRobotDatasetVersion::V2 | LeRobotDatasetVersion::V3 => {
                Self::load_dataset(settings, filepath, tx)
            }
        }
    }

    fn import_from_file_contents(
        &self,
        _settings: &crate::ImporterSettings,
        filepath: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: Sender<ImportedData>,
    ) -> Result<(), ImporterError> {
        Err(ImporterError::Incompatible(filepath))
    }
}

impl LeRobotDatasetImporter {
    fn load_dataset(
        settings: &crate::ImporterSettings,
        filepath: impl AsRef<std::path::Path>,
        tx: Sender<ImportedData>,
    ) -> Result<(), ImporterError> {
        let filepath = filepath.as_ref().to_owned();
        let dataset = LeRobotDataset::open(&filepath)
            .map_err(|err| anyhow!("Loading LeRobot dataset failed: {err}"))?;

        let application_id = settings
            .application_id
            .clone()
            .unwrap_or_else(|| ApplicationId::new_or_unknown(filepath.display().to_string()));

        let loader_name = Self.name();

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of importers on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        thread::Builder::new()
            .name(format!("load_and_stream({filepath:?})"))
            .spawn(move || {
                re_log::info!(
                    "Loading LeRobot dataset from {:?}, with {} episode(s)",
                    dataset.path(),
                    dataset.episodes().count(),
                );
                let config = LeRobotConfig::default();
                load_and_stream(&dataset, &config, &application_id, &tx, &loader_name);
            })
            .with_context(|| {
                format!("Failed to spawn IO thread to load LeRobot dataset {filepath:?}")
            })?;

        Ok(())
    }
}

/// Send `SetStoreInfo` messages for each episode and return the associated store ids.
fn prepare_episode_chunks(
    episodes: impl IntoIterator<Item = EpisodeIndex>,
    application_id: &ApplicationId,
    tx: &Sender<ImportedData>,
    loader_name: &str,
) -> Vec<(EpisodeIndex, StoreId)> {
    let mut store_ids = vec![];

    for episode in episodes {
        let store_id = StoreId::recording(application_id.clone(), format!("episode_{}", episode.0));
        let set_store_info = ImportedData::LogMsg(
            loader_name.to_owned(),
            prepare_store_info(&store_id, re_log_types::FileSource::Sdk),
        );

        if send_crossbeam(tx, set_store_info).is_err() {
            break;
        }

        store_ids.push((episode, store_id));
    }

    store_ids
}

/// Prepare store info for all episodes and stream them one at a time.
///
/// Guarantees the two-phase protocol the viewer relies on: one `SetStoreInfo` per episode,
/// all sent (in ascending episode order) before any chunk data is streamed.
///
/// Chunks are forwarded as each episode's stream yields them. An episode that fails to
/// stream at all is skipped with a warning; so is any single failed feature within one.
fn load_and_stream(
    dataset: &LeRobotDataset,
    config: &LeRobotConfig,
    application_id: &ApplicationId,
    tx: &Sender<ImportedData>,
    loader_name: &str,
) {
    let store_ids = prepare_episode_chunks(dataset.episodes(), application_id, tx, loader_name);

    for (episode, store_id) in &store_ids {
        match dataset.stream(*episode, config) {
            Ok(chunks) => {
                let recording_info = re_sdk_types::archetypes::RecordingInfo::new()
                    .with_name(format!("Episode {}", episode.0));

                let Ok(initial) = Chunk::builder(EntityPath::properties())
                    .with_archetype(RowId::new(), TimePoint::STATIC, &recording_info)
                    .build()
                else {
                    re_log::error!(
                        "Failed to build recording properties chunk for episode {}",
                        episode.0
                    );
                    continue;
                };

                for result in std::iter::chain(std::iter::once(Ok(initial)), chunks) {
                    match result {
                        Ok(chunk) => {
                            let data = ImportedData::Chunk(
                                loader_name.to_owned(),
                                store_id.clone(),
                                chunk,
                            );

                            if send_crossbeam(tx, data).is_err() {
                                break;
                            }
                        }
                        Err(err) => {
                            re_log::warn!(
                                "Failed to load a feature of episode {} from LeRobot dataset: {err}",
                                episode.0
                            );
                        }
                    }
                }
            }
            Err(err) => {
                re_log::warn!(
                    "Failed to load episode {} from LeRobot dataset: {err}",
                    episode.0
                );
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use re_log_types::LogMsg;

    use super::*;

    /// Everything the importer emitted, in emission order.
    #[derive(Debug)]
    enum Event {
        StoreInfo(StoreId),
        Chunk(StoreId, String),
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/assets/lerobot")
            .join(name)
    }

    fn import_dataset(path: &Path) -> Vec<Event> {
        let (tx, rx) = crossbeam::channel::bounded(1);
        LeRobotDatasetImporter
            .import_from_path(
                &crate::ImporterSettings::recommended("lerobot_test"),
                path.to_owned(),
                tx,
            )
            .expect("dataset should start importing");

        rx.into_iter()
            .filter_map(|data| match data {
                ImportedData::LogMsg(_, LogMsg::SetStoreInfo(info)) => {
                    Some(Event::StoreInfo(info.info.store_id))
                }
                ImportedData::Chunk(_, store_id, chunk) => {
                    Some(Event::Chunk(store_id, chunk.entity_path().to_string()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn imports_real_v2_and_v3_datasets_into_one_recording_per_episode() {
        for (fixture_name, expected_version) in [
            ("v21_apple_storage", LeRobotDatasetVersion::V2),
            ("v30_apple_storage", LeRobotDatasetVersion::V3),
        ] {
            let path = fixture(fixture_name);
            assert_eq!(
                LeRobotDatasetVersion::find_version(&path),
                Some(expected_version)
            );

            let events = import_dataset(&path);

            // Two-phase protocol: every `SetStoreInfo` precedes any chunk.
            let last_store_info = events
                .iter()
                .rposition(|event| matches!(event, Event::StoreInfo(_)))
                .expect("at least one SetStoreInfo");
            let first_chunk = events
                .iter()
                .position(|event| matches!(event, Event::Chunk(..)))
                .expect("at least one chunk");
            assert!(
                last_store_info < first_chunk,
                "{fixture_name}: all SetStoreInfo must be sent before any chunk"
            );

            // One recording per episode, announced in ascending order, all for this dataset.
            let store_ids: Vec<&StoreId> = events
                .iter()
                .filter_map(|event| match event {
                    Event::StoreInfo(store_id) => Some(store_id),
                    Event::Chunk(..) => None,
                })
                .collect();
            assert_eq!(
                store_ids
                    .iter()
                    .map(|store_id| store_id.recording_id().as_str())
                    .collect::<Vec<_>>(),
                ["episode_0", "episode_1", "episode_2"]
            );
            assert!(store_ids.iter().all(|store_id| {
                store_id.is_recording()
                    && store_id.application_id() == store_ids[0].application_id()
            }));

            // Each recording starts with its properties chunk and contains the expected entities.
            let mut entity_paths_by_recording = BTreeMap::<String, Vec<&str>>::new();
            for event in &events {
                if let Event::Chunk(store_id, entity_path) = event {
                    entity_paths_by_recording
                        .entry(store_id.recording_id().as_str().to_owned())
                        .or_default()
                        .push(entity_path);
                }
            }
            // v3 video streams through an ffmpeg transcode (its episode window is sliced
            // by ffmpeg), so `/observation.image` only appears when ffmpeg is available;
            // the importer warn-and-skips the feature otherwise.
            let expect_video = expected_version == LeRobotDatasetVersion::V2
                || std::process::Command::new("ffmpeg")
                    .arg("-version")
                    .output()
                    .is_ok_and(|output| output.status.success());
            let mut expected_paths = vec!["/action", "/observation.state", "/task"];
            if expect_video {
                expected_paths.push("/observation.image");
            }

            for (recording_id, entity_paths) in &entity_paths_by_recording {
                assert_eq!(
                    entity_paths.first().copied(),
                    Some("/__properties"),
                    "{fixture_name} {recording_id} must start with its properties chunk"
                );
                // Exactly the importer's own chunk: the parquet file's key-value
                // metadata (a pandas schema) must not leak onto the same entity.
                assert_eq!(
                    entity_paths
                        .iter()
                        .filter(|path| **path == "/__properties")
                        .count(),
                    1,
                    "{fixture_name} {recording_id} must have exactly one properties chunk; got {entity_paths:?}"
                );
                for expected_path in &expected_paths {
                    assert!(
                        entity_paths.contains(expected_path),
                        "{fixture_name} {recording_id} is missing {expected_path}; got {entity_paths:?}"
                    );
                }
            }
            assert_eq!(entity_paths_by_recording.len(), 3, "{fixture_name}");
        }
    }
}
