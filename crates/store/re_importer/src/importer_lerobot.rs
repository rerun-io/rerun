use std::thread;

use anyhow::{Context as _, anyhow};
use crossbeam::channel::Sender;
use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_log_types::{ApplicationId, StoreId};
use re_quota_channel::send_crossbeam;

use crate::{ImportedData, Importer, ImporterError, import_file::prepare_store_info};
use re_lerobot::{
    EpisodeIndex, LeRobotDatasetVersion, LeRobotError, common::LeRobotDataset, datasetv2,
    datasetv3, is_lerobot_dataset,
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
                load_and_stream_versioned(&dataset, &application_id, &tx, &loader_name);
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
                load_and_stream_versioned(&dataset, &application_id, &tx, &loader_name);
            })
            .with_context(|| {
                format!("Failed to spawn IO thread to load LeRobot v3 dataset {filepath:?}")
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

/// Shared streaming loop for `LeRobot` dataset versions.
fn load_and_stream_common<Dataset>(
    dataset: &Dataset,
    store_ids: &[(EpisodeIndex, StoreId)],
    tx: &Sender<ImportedData>,
    loader_name: &str,
    load_episode: impl Fn(&Dataset, EpisodeIndex) -> Result<Vec<Chunk>, LeRobotError>,
) {
    for (episode, store_id) in store_ids {
        // log episode data to its respective recording
        match load_episode(dataset, *episode) {
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
                    return;
                };

                for chunk in std::iter::chain(std::iter::once(initial), chunks) {
                    let data = ImportedData::Chunk(loader_name.to_owned(), store_id.clone(), chunk);

                    if send_crossbeam(tx, data).is_err() {
                        break; // The other end has decided to hang up, not our problem.
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

/// Prepare store info for all episodes and stream them using the provided loader.
///
/// Guarantees the two-phase protocol the viewer relies on: one `SetStoreInfo` per episode,
/// all sent (in ascending episode order) before any chunk data is streamed.
fn load_and_stream_versioned<D: LeRobotDataset>(
    dataset: &D,
    application_id: &ApplicationId,
    tx: &Sender<ImportedData>,
    loader_name: &str,
) {
    let store_ids = prepare_episode_chunks(
        dataset.iter_episode_indices(),
        application_id,
        tx,
        loader_name,
    );
    load_and_stream_common(dataset, &store_ids, tx, loader_name, |dataset, episode| {
        dataset.load_episode_chunks(episode)
    });
}
#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    use re_log_types::LogMsg;

    use super::*;

    use re_sdk_types::archetypes::TextDocument;

    struct TestDataset {
        episodes: Vec<EpisodeIndex>,
    }

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

    impl LeRobotDataset for TestDataset {
        fn iter_episode_indices(&self) -> impl Iterator<Item = EpisodeIndex> {
            self.episodes.iter().copied()
        }

        fn load_episode_chunks(&self, episode: EpisodeIndex) -> Result<Vec<Chunk>, LeRobotError> {
            let chunk = Chunk::builder(format!("episode_{}", episode.0))
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &TextDocument::new(format!("Episode {}", episode.0)),
                )
                .build()?;
            Ok(vec![chunk])
        }
    }

    #[test]
    fn streams_each_episode_to_its_own_recording() {
        let dataset = TestDataset {
            episodes: (0..3).map(EpisodeIndex).collect(),
        };
        let application_id = ApplicationId::from("lerobot_test");
        let loader_name = "rerun.importers.LeRobotDataset";
        // The loader and receiver run on this test thread, so the queue cannot grow independently.
        #[expect(
            clippy::disallowed_methods,
            reason = "the sender and receiver are on the same thread"
        )]
        let (tx, rx) = crossbeam::channel::unbounded();

        load_and_stream_versioned(&dataset, &application_id, &tx, loader_name);
        drop(tx);

        let imported = rx.into_iter().collect::<Vec<_>>();
        assert!(
            imported
                .iter()
                .all(|data| data.importer_name() == loader_name)
        );

        let store_info_ids = imported
            .iter()
            .filter_map(|data| match data {
                ImportedData::LogMsg(_, re_log_types::LogMsg::SetStoreInfo(store_info)) => {
                    Some(store_info.info.store_id.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            store_info_ids
                .iter()
                .map(|store_id| store_id.recording_id().as_str())
                .collect::<Vec<_>>(),
            ["episode_0", "episode_1", "episode_2"]
        );
        assert!(store_info_ids.iter().all(|store_id| {
            store_id.is_recording() && store_id.application_id() == &application_id
        }));

        let streamed_chunks = imported
            .iter()
            .filter_map(|data| match data {
                ImportedData::Chunk(_, store_id, chunk) => Some((
                    store_id.recording_id().as_str(),
                    chunk.entity_path().to_string(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            streamed_chunks,
            [
                ("episode_0", EntityPath::properties().to_string()),
                ("episode_0", "/episode_0".to_owned()),
                ("episode_1", EntityPath::properties().to_string()),
                ("episode_1", "/episode_1".to_owned()),
                ("episode_2", EntityPath::properties().to_string()),
                ("episode_2", "/episode_2".to_owned()),
            ]
        );
    }
}
