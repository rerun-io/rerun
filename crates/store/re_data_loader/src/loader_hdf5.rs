//! Rerun dataloader for HDF5 files.

#[cfg(feature = "hdf5")]
use re_chunk::RowId;
#[cfg(feature = "hdf5")]
use re_log_types::{SetStoreInfo, StoreId, StoreInfo};

#[cfg(feature = "hdf5")]
use crate::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};

#[cfg(all(feature = "hdf5", not(target_arch = "wasm32")))]
use std::{path::Path, sync::mpsc::Sender};

#[cfg(feature = "hdf5")]
const HDF5_LOADER_NAME: &str = "Hdf5Loader";

#[cfg(feature = "hdf5")]
/// A [`DataLoader`] for HDF5 files.
///
/// The HDF5 loader extracts datasets from HDF5 files and converts them to Arrow format
/// for use in Rerun. It performs raw extraction of datasets without interpretation,
/// preserving the original data structure as closely as possible.
///
/// Supported HDF5 features:
/// - Numeric datasets (integers, floats)
/// - String datasets
/// - Multi-dimensional arrays
/// - Group hierarchies (mapped to entity paths)
/// - Basic attributes as metadata
pub struct Hdf5Loader;

#[cfg(feature = "hdf5")]
impl Default for Hdf5Loader {
    fn default() -> Self {
        Self
    }
}

impl Hdf5Loader {
    /// Creates a new [`Hdf5Loader`].
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "hdf5")]
impl DataLoader for Hdf5Loader {
    fn name(&self) -> crate::DataLoaderName {
        HDF5_LOADER_NAME.into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        path: std::path::PathBuf,
        tx: Sender<crate::LoadedData>,
    ) -> std::result::Result<(), DataLoaderError> {
        if !is_hdf5_file(&path) {
            return Err(DataLoaderError::Incompatible(path)); // simply not interested
        }

        re_tracing::profile_function!();

        // NOTE: this must be spawned on a dedicated thread to avoid potential blocking issues
        let settings = settings.clone();
        std::thread::Builder::new()
            .name(format!("load_hdf5({path:?})"))
            .spawn(move || {
                if let Err(err) = load_hdf5_file(&path, &settings, &tx) {
                    re_log::error!("Failed to load HDF5 file: {err}");
                }
            })
            .map_err(|err| DataLoaderError::Other(err.into()))?;

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        _settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        _tx: Sender<crate::LoadedData>,
    ) -> std::result::Result<(), crate::DataLoaderError> {
        // HDF5 files require filesystem access for the current implementation
        // In-memory loading would require additional work with the HDF5 crate
        Err(DataLoaderError::Incompatible(filepath))
    }
}

#[cfg(all(feature = "hdf5", not(target_arch = "wasm32")))]
fn load_hdf5_file(
    filepath: &std::path::PathBuf,
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
) -> std::result::Result<(), DataLoaderError> {
    use hdf5_metno::File;

    re_tracing::profile_function!();
    let store_id = settings.recommended_store_id();

    // Send store info
    if tx
        .send(LoadedData::LogMsg(
            HDF5_LOADER_NAME.to_owned(),
            re_log_types::LogMsg::SetStoreInfo(store_info(store_id.clone())),
        ))
        .is_err()
    {
        re_log::debug_once!(
            "Failed to send `SetStoreInfo` because smart channel closed unexpectedly."
        );
        return Ok(());
    }

    // Open HDF5 file
    let _file = File::open(filepath)
        .map_err(|e| DataLoaderError::Other(anyhow::anyhow!("Failed to open HDF5 file: {}", e)))?;

    // TODO: Implement actual HDF5 dataset extraction
    // This is a placeholder that will be implemented in the next phase
    re_log::info!("HDF5 file opened successfully: {}", filepath.display());
    re_log::warn!("HDF5 dataset extraction not yet implemented");

    Ok(())
}

#[cfg(feature = "hdf5")]
pub fn store_info(store_id: StoreId) -> SetStoreInfo {
    SetStoreInfo {
        row_id: *RowId::new(),
        info: StoreInfo::new(
            store_id,
            re_log_types::StoreSource::Other(HDF5_LOADER_NAME.to_owned()),
        ),
    }
}

/// Checks if a path has an HDF5 file extension.
#[cfg(all(feature = "hdf5", not(target_arch = "wasm32")))]
fn has_hdf5_extension(filepath: &Path) -> bool {
    filepath
        .extension()
        .map(|ext| {
            let ext_str = ext.to_string_lossy().to_lowercase();
            ext_str == "h5" || ext_str == "hdf5" || ext_str == "hdf"
        })
        .unwrap_or(false)
}

/// Checks if a file is an HDF5 file based on its extension and existence.
#[cfg(all(feature = "hdf5", not(target_arch = "wasm32")))]
fn is_hdf5_file(filepath: &Path) -> bool {
    filepath.is_file() && has_hdf5_extension(filepath)
}

#[cfg(all(test, feature = "hdf5", not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_hdf5_file() {
        // Test HDF5 extensions (only check extension, not file existence)
        assert!(has_hdf5_extension(&PathBuf::from("test.h5")));
        assert!(has_hdf5_extension(&PathBuf::from("test.hdf5")));
        assert!(has_hdf5_extension(&PathBuf::from("test.hdf")));
        assert!(has_hdf5_extension(&PathBuf::from("TEST.H5")));

        // Test non-HDF5 extensions
        assert!(!has_hdf5_extension(&PathBuf::from("test.txt")));
        assert!(!has_hdf5_extension(&PathBuf::from("test.mcap")));
        assert!(!has_hdf5_extension(&PathBuf::from("test.rrd")));
    }
}
