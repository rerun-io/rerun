use std::thread;

use anyhow::{Context as _, anyhow};
use crossbeam::channel::Sender;
use re_log_types::ApplicationId;

use crate::lerobot::{LeRobotDatasetVersion, datasetv2, datasetv3, is_lerobot_dataset};
use crate::{ImportedData, Importer, ImporterError};

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
            LeRobotDatasetVersion::V1 => {
                re_log::error!("LeRobot 'v1.x' dataset format is unsupported.");
                Ok(())
            }
            LeRobotDatasetVersion::V2 => Self::load_v2_dataset(settings, filepath, tx),
            LeRobotDatasetVersion::V3 => Self::load_v3_dataset(settings, filepath, tx),
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
    fn load_v2_dataset(
        settings: &crate::ImporterSettings,
        filepath: impl AsRef<std::path::Path>,
        tx: Sender<ImportedData>,
    ) -> Result<(), ImporterError> {
        let filepath = filepath.as_ref().to_owned();
        let dataset = datasetv2::LeRobotDatasetV2::load_from_directory(&filepath)
            .map_err(|err| anyhow!("Loading LeRobot v2 dataset failed: {err}"))?;

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
            .name(format!("load_and_stream_v2({filepath:?})"))
            .spawn(move || {
                re_log::info!(
                    "Loading LeRobot v2 dataset from {:?}, with {} episode(s)",
                    dataset.path,
                    dataset.metadata.episode_count(),
                );
                datasetv2::load_and_stream(&dataset, &application_id, &tx, &loader_name);
            })
            .with_context(|| {
                format!("Failed to spawn IO thread to load LeRobot v2 dataset {filepath:?}")
            })?;

        Ok(())
    }

    fn load_v3_dataset(
        settings: &crate::ImporterSettings,
        filepath: impl AsRef<std::path::Path>,
        tx: Sender<ImportedData>,
    ) -> Result<(), ImporterError> {
        let filepath = filepath.as_ref().to_owned();
        let dataset = datasetv3::LeRobotDatasetV3::load_from_directory(&filepath)
            .map_err(|err| anyhow!("Loading LeRobot v3 dataset failed: {err}"))?;

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
            .name(format!("load_and_stream_v3({filepath:?})"))
            .spawn(move || {
                re_log::info!(
                    "Loading LeRobot v3 dataset from {:?}, with {} episode(s)",
                    dataset.path,
                    dataset.metadata.episode_count(),
                );
                datasetv3::load_and_stream(&dataset, &application_id, &tx, &loader_name);
            })
            .with_context(|| {
                format!("Failed to spawn IO thread to load LeRobot v3 dataset {filepath:?}")
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    use re_log_types::LogMsg;

    use super::*;

    #[derive(Debug)]
    struct ImportSummary {
        recording_ids: Vec<String>,
        entity_paths_by_recording: BTreeMap<String, BTreeSet<String>>,
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/assets/lerobot")
            .join(name)
    }

    fn import_dataset(path: &Path) -> ImportSummary {
        let (tx, rx) = crossbeam::channel::bounded(1);
        LeRobotDatasetImporter
            .import_from_path(
                &crate::ImporterSettings::recommended("lerobot_test"),
                path.to_owned(),
                tx,
            )
            .expect("dataset should start importing");

        let mut recording_ids = Vec::new();
        let mut entity_paths_by_recording = BTreeMap::<_, BTreeSet<_>>::new();
        for data in rx {
            match data {
                ImportedData::LogMsg(_, LogMsg::SetStoreInfo(info)) => {
                    recording_ids.push(info.info.store_id.recording_id().as_str().to_owned());
                }
                ImportedData::Chunk(_, store_id, chunk) => {
                    entity_paths_by_recording
                        .entry(store_id.recording_id().as_str().to_owned())
                        .or_default()
                        .insert(chunk.entity_path().to_string());
                }
                _ => {}
            }
        }

        ImportSummary {
            recording_ids,
            entity_paths_by_recording,
        }
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

            let imported = import_dataset(&path);
            assert_eq!(
                imported.recording_ids,
                ["episode_0", "episode_1", "episode_2"]
            );
            for recording_id in &imported.recording_ids {
                let entity_paths = &imported.entity_paths_by_recording[recording_id];
                for expected_path in [
                    "/__properties",
                    "/action",
                    "/observation.state",
                    "/observation.image",
                    "/task",
                ] {
                    assert!(
                        entity_paths.contains(expected_path),
                        "{fixture_name} {recording_id} is missing {expected_path}; got {entity_paths:?}"
                    );
                }
            }
        }
    }
}
