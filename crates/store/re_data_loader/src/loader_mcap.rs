//! Rerun dataloader for MCAP files.

use std::collections::BTreeSet;
use std::{io::Cursor, sync::mpsc::Sender};

use anyhow::Context as _;
use re_chunk::RowId;
use re_log_types::{SetStoreInfo, StoreId, StoreInfo};

use crate::mcap::layers::LayerRegistry;
use crate::mcap::{self, layers};
use crate::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};

const MCAP_LOADER_NAME: &str = "McapLoader";

#[derive(Default)]
pub struct McapLoader {
    layer_filters: Option<BTreeSet<String>>,
}

impl McapLoader {
    /// Creates a new [`McapLoader`] that only extracts the specified `layers`.
    pub fn with_layers(layers: impl IntoIterator<Item = String>) -> Self {
        Self {
            layer_filters: Some(layers.into_iter().collect()),
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
        if path.is_dir()
            || path
                .extension()
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("mcap"))
        {
            return Err(DataLoaderError::Incompatible(path)); // simply not interested
        }

        re_tracing::profile_function!();

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        let settings = settings.clone();
        let layer_filters = self.layer_filters.clone();
        std::thread::Builder::new()
            .name(format!("load_mcap({path:?}"))
            .spawn(
                move || match load_mcap_mmap(&path, &settings, &tx, layer_filters) {
                    Ok(_) => {}
                    Err(err) => {
                        re_log::error!("Failed to load MCAP file: {err}");
                    }
                },
            )
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
        if filepath.is_dir() || filepath.extension().is_none_or(|ext| ext != "mcap") {
            return Err(DataLoaderError::Incompatible(filepath)); // simply not interested
        }

        re_tracing::profile_function!();

        let settings = settings.clone();
        let layer_filters = self.layer_filters.clone();

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        std::thread::Builder::new()
            .name(format!("load_mcap({filepath:?}"))
            .spawn(
                move || match load_mcap_mmap(&filepath, &settings, &tx, layer_filters) {
                    Ok(_) => {}
                    Err(err) => {
                        re_log::error!("Failed to load MCAP file: {err}");
                    }
                },
            )
            .map_err(|err| DataLoaderError::Other(err.into()))?;

        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn load_from_file_contents(
        &self,
        settings: &crate::DataLoaderSettings,
        _filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: Sender<crate::LoadedData>,
    ) -> std::result::Result<(), DataLoaderError> {
        let contents = contents.into_owned();

        load_mcap(&contents, settings, &tx, None)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_mcap_mmap(
    filepath: &std::path::PathBuf,
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
    layer_filters: Option<BTreeSet<String>>,
) -> std::result::Result<(), DataLoaderError> {
    use std::fs::File;
    let file = File::open(filepath)?;

    // SAFETY: file-backed memory maps are marked unsafe because of potential UB when using the map and the underlying file is modified.
    #[allow(unsafe_code)]
    let mmap = unsafe { memmap2::Mmap::map(&file)? };

    load_mcap(&mmap, settings, tx, layer_filters)
}

fn load_mcap(
    mcap: &[u8],
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
    layer_filters: Option<BTreeSet<String>>,
) -> Result<(), DataLoaderError> {
    re_tracing::profile_function!();

    let store_id = settings.recommended_store_id();

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
                McapLoader {
                    layer_filters: None,
                }
                .name(),
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

    let summary = mcap::util::read_summary(reader)?
        .ok_or_else(|| anyhow::anyhow!("MCAP file does not contain a summary"))?;

    let registry = LayerRegistry::default()
        .register::<layers::McapProtobufLayer>()
        .register::<layers::McapRawLayer>()
        .register::<layers::McapRecordingInfoLayer>()
        .register::<layers::McapRos2Layer>()
        .register::<layers::McapSchemaLayer>()
        .register::<layers::McapStatisticLayer>();

    // TODO(#10862): Add warning for channel that miss semantic information.

    let mut empty = true;
    for mut layer in registry.layers(layer_filters) {
        re_tracing::profile_scope!("process-layer");
        empty &= false;
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
            store_source: re_log_types::StoreSource::Other(
                McapLoader {
                    layer_filters: None,
                }
                .name(),
            ),
            store_version: Some(re_build_info::CrateVersion::LOCAL),
        },
    }
}
