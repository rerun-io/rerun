use std::thread;

use anyhow::{Context as _, anyhow};
use crossbeam::channel::Sender;

use crate::lerobot::{LeRobotDatasetVersion, datasetv2, datasetv3, is_lerobot_dataset};
use crate::{DataLoader, DataLoaderError, LoadedData};

/// A [`DataLoader`] for `LeRobot` datasets.
///
/// An example dataset which can be loaded can be found on Hugging Face: [lerobot/pusht_image](https://huggingface.co/datasets/lerobot/pusht_image)
pub struct LeRobotDatasetLoader;

impl DataLoader for LeRobotDatasetLoader {
    fn name(&self) -> String {
        "LeRobotDatasetLoader".into()
    }

    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        tx: Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        if !is_lerobot_dataset(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
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

    fn load_from_file_contents(
        &self,
        _settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        Err(DataLoaderError::Incompatible(filepath))
    }
}

impl LeRobotDatasetLoader {
    fn load_v2_dataset(
        settings: &crate::DataLoaderSettings,
        filepath: impl AsRef<std::path::Path>,
        tx: Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        let filepath = filepath.as_ref().to_owned();
        let dataset = datasetv2::LeRobotDatasetV2::load_from_directory(&filepath)
            .map_err(|err| anyhow!("Loading LeRobot v2 dataset failed: {err}"))?;

        let application_id = settings
            .application_id
            .clone()
            .unwrap_or_else(|| filepath.display().to_string().into());

        let loader_name = Self.name();

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
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
        settings: &crate::DataLoaderSettings,
        filepath: impl AsRef<std::path::Path>,
        tx: Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        let filepath = filepath.as_ref().to_owned();
        let dataset = datasetv3::LeRobotDatasetV3::load_from_directory(&filepath)
            .map_err(|err| anyhow!("Loading LeRobot v3 dataset failed: {err}"))?;

        let application_id = settings
            .application_id
            .clone()
            .unwrap_or_else(|| filepath.display().to_string().into());

        let loader_name = Self.name();

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
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
