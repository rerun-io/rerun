//! Rerun dataloader for MCAP files.

use std::{io::Cursor, path::Path, sync::mpsc::Sender};

use anyhow::Context as _;
use re_chunk::RowId;
use re_log_types::{SetStoreInfo, StoreId, StoreInfo};
use re_mcap::{LayerRegistry, SelectedLayers};

use crate::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};

const MCAP_LOADER_NAME: &str = "McapLoader";

/// A [`DataLoader`] for MCAP files.
///
/// There are many different ways to extract and interpret information from MCAP files.
/// For example, it might be interesting to query for particular fields of messages,
/// or show information directly in the Rerun viewer. Because use-cases can vary, the
/// [`McapLoader`] is made up of [`re_mcap::Layer`]s, each representing different views of the
/// underlying data.
///
/// These layers can be specified in the CLI wen converting an MCAP file
/// to an .rrd. Here are a few examples:
/// - [`re_mcap::layers::McapProtobufLayer`]
/// - [`re_mcap::layers::McapRawLayer`]
pub struct McapLoader {
    selected_layers: SelectedLayers,
    raw_fallback_enabled: bool,
}

impl Default for McapLoader {
    fn default() -> Self {
        Self {
            selected_layers: SelectedLayers::All,
            raw_fallback_enabled: true,
        }
    }
}

impl McapLoader {
    /// Creates a new [`McapLoader`] that only extracts the specified `layers`.
    pub fn new(selected_layers: SelectedLayers) -> Self {
        Self {
            selected_layers,
            raw_fallback_enabled: true,
        }
    }

    /// Creates a new [`McapLoader`] with configurable raw fallback.
    pub fn with_raw_fallback(selected_layers: SelectedLayers, raw_fallback_enabled: bool) -> Self {
        Self {
            selected_layers,
            raw_fallback_enabled,
        }
    }
}

impl DataLoader for McapLoader {
    fn name(&self) -> crate::DataLoaderName {
        MCAP_LOADER_NAME.into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_path(
        &self,
        settings: &crate::DataLoaderSettings,
        path: std::path::PathBuf,
        tx: Sender<crate::LoadedData>,
    ) -> std::result::Result<(), DataLoaderError> {
        if !is_mcap_file(&path) {
            return Err(DataLoaderError::Incompatible(path)); // simply not interested
        }

        re_tracing::profile_function!();

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        let settings = settings.clone();
        let selected_layers = self.selected_layers.clone();
        let raw_fallback_enabled = self.raw_fallback_enabled;
        std::thread::Builder::new()
            .name(format!("load_mcap({path:?}"))
            .spawn(move || {
                match load_mcap_mmap(&path, &settings, &tx, selected_layers, raw_fallback_enabled) {
                    Ok(_) => {}
                    Err(err) => {
                        re_log::error!("Failed to load MCAP file: {err}");
                    }
                }
            })
            .map_err(|err| DataLoaderError::Other(err.into()))?;

        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_from_file_contents(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        _contents: std::borrow::Cow<'_, [u8]>,
        tx: Sender<crate::LoadedData>,
    ) -> std::result::Result<(), crate::DataLoaderError> {
        if !is_mcap_file(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath)); // simply not interested
        }

        re_tracing::profile_function!();

        let settings = settings.clone();
        let selected_layers = self.selected_layers.clone();
        let raw_fallback_enabled = self.raw_fallback_enabled;

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        std::thread::Builder::new()
            .name(format!("load_mcap({filepath:?}"))
            .spawn(move || {
                match load_mcap_mmap(
                    &filepath,
                    &settings,
                    &tx,
                    selected_layers,
                    raw_fallback_enabled,
                ) {
                    Ok(_) => {}
                    Err(err) => {
                        re_log::error!("Failed to load MCAP file: {err}");
                    }
                }
            })
            .map_err(|err| DataLoaderError::Other(err.into()))?;

        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn load_from_file_contents(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: Sender<crate::LoadedData>,
    ) -> std::result::Result<(), DataLoaderError> {
        if !is_mcap_file(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath)); // simply not interested
        }

        let contents = contents.into_owned();

        load_mcap(
            &contents,
            settings,
            &tx,
            self.selected_layers.clone(),
            self.raw_fallback_enabled,
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_mcap_mmap(
    filepath: &std::path::PathBuf,
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
    selected_layers: SelectedLayers,
    raw_fallback_enabled: bool,
) -> std::result::Result<(), DataLoaderError> {
    use std::fs::File;
    let file = File::open(filepath)?;

    // SAFETY: file-backed memory maps are marked unsafe because of potential UB when using the map and the underlying file is modified.
    #[allow(unsafe_code)]
    let mmap = unsafe { memmap2::Mmap::map(&file)? };

    load_mcap(&mmap, settings, tx, selected_layers, raw_fallback_enabled)
}

fn load_mcap(
    mcap: &[u8],
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
    selected_layers: SelectedLayers,
    raw_fallback_enabled: bool,
) -> Result<(), DataLoaderError> {
    re_tracing::profile_function!();

    // If there's an open store, use it. If there's no store yet, use the recommended id.
    let store_id = settings.opened_store_id_or_recommended();

    if tx
        .send(LoadedData::LogMsg(
            MCAP_LOADER_NAME.to_owned(),
            re_log_types::LogMsg::SetStoreInfo(store_info(store_id.clone())),
        ))
        .is_err()
    {
        re_log::debug_once!(
            "Failed to send `SetStoreInfo` because smart channel closed unexpectedly."
        );
        // If the other side decided to hang up this is not our problem.
        return Ok(());
    }

    let mut send_chunk = |chunk| {
        if tx
            .send(LoadedData::Chunk(
                MCAP_LOADER_NAME.to_owned(),
                store_id.clone(),
                chunk,
            ))
            .is_err()
        {
            // If the other side decided to hang up this is not our problem.
            re_log::debug_once!(
                "Failed to send chunk because the smart channel has been closed unexpectedly."
            );
        }
    };

    let reader = Cursor::new(&mcap);

    let summary = re_mcap::read_summary(reader)?
        .ok_or_else(|| anyhow::anyhow!("MCAP file does not contain a summary"))?;

    let registry = LayerRegistry::all_with_raw_fallback(raw_fallback_enabled);

    // TODO(#10862): Add warning for channel that miss semantic information.

    let mut empty = true;
    for mut layer in registry.layers(selected_layers) {
        re_tracing::profile_scope!("process-layer");
        empty = false;
        layer
            .process(mcap, &summary, &mut send_chunk)
            .with_context(|| "processing layers")?;
    }
    if empty {
        re_log::warn_once!("No layers were selected");
    }

    Ok(())
}

pub fn store_info(store_id: StoreId) -> SetStoreInfo {
    SetStoreInfo {
        row_id: *RowId::new(),
        info: StoreInfo {
            store_id,
            cloned_from: None,
            store_source: re_log_types::StoreSource::Other(MCAP_LOADER_NAME.to_owned()),
            store_version: Some(re_build_info::CrateVersion::LOCAL),
        },
    }
}

/// Checks if a file is an MCAP file.
fn is_mcap_file(filepath: &Path) -> bool {
    !filepath.is_dir()
        && filepath
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("mcap"))
            .unwrap_or(false)
}
