use crate::le_robot::LeRobotDatasetMetadata;
use crate::{DataLoader, DataLoaderError, LoadedData};

pub struct LeRobotDatasetLoader;

impl DataLoader for LeRobotDatasetLoader {
    fn name(&self) -> String {
        "LeRobotDatasetLoader".into()
    }

    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        if !crate::le_robot::is_le_robot_dataset(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath));
        }

        let metadata = LeRobotDatasetMetadata::load(&filepath);
        Ok(())
    }

    fn load_from_file_contents(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: std::sync::mpsc::Sender<LoadedData>,
    ) -> Result<(), DataLoaderError> {
        todo!()
    }
}
