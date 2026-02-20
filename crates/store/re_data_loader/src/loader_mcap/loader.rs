//! MCAP file loader implementation.

use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use crossbeam::channel::Sender;
use re_chunk::RowId;
use re_lenses::Lenses;
use re_log_types::{SetStoreInfo, StoreId, StoreInfo};
use re_mcap::{LayerIdentifier, LayerRegistry, SelectedLayers};

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
///
/// Optionally, [`Lenses`] can be configured via [`Self::with_lenses`] to transform
/// chunks as they are loaded (e.g., converting raw protobuf data into semantic Rerun components).
pub struct McapLoader {
    selected_layers: SelectedLayers,
    // TODO(RR-3491): We don't need the fallback logic anymore; use `OutputMode` instead.
    raw_fallback_enabled: bool,
    lenses: Option<Arc<Lenses>>,
}

impl Default for McapLoader {
    fn default() -> Self {
        Self::new(SelectedLayers::All)
    }
}

impl McapLoader {
    /// Creates a new [`McapLoader`] that extracts the specified `layers`.
    pub fn new(selected_layers: SelectedLayers) -> Self {
        let lenses = Self::build_lenses(&selected_layers);
        Self {
            selected_layers,
            raw_fallback_enabled: true,
            lenses,
        }
    }

    /// Configures whether the raw layer is used as a fallback for unsupported channels.
    pub fn with_raw_fallback(mut self, raw_fallback_enabled: bool) -> Self {
        self.raw_fallback_enabled = raw_fallback_enabled;
        self
    }

    /// Configures lenses to apply to chunks as they are loaded.
    pub fn with_lenses(mut self, lenses: Lenses) -> Self {
        self.lenses = Some(Arc::new(lenses));
        self
    }

    fn build_lenses(selected_layers: &SelectedLayers) -> Option<Arc<Lenses>> {
        if !selected_layers.contains(&LayerIdentifier::from(
            super::lenses::FOXGLOVE_LENSES_IDENTIFIER,
        )) {
            return None;
        }

        match super::lenses::foxglove_lenses() {
            Ok(lenses) => Some(Arc::new(lenses)),
            Err(err) => {
                re_log::error_once!(
                    "Failed to build Foxglove lenses: {err}. MCAP loader will run without them."
                );
                None
            }
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
    ) -> Result<(), DataLoaderError> {
        if !path.is_file() || !has_mcap_extension(&path) {
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
        let lenses = self.lenses.clone();
        std::thread::Builder::new()
            .name(format!("load_mcap({path:?}"))
            .spawn(move || {
                if let Err(err) = load_mcap_mmap(
                    &path,
                    &settings,
                    &tx,
                    &selected_layers,
                    raw_fallback_enabled,
                    lenses.as_deref(),
                ) {
                    re_log::error!("Failed to load MCAP file: {err}");
                }
            })
            .map_err(|err| DataLoaderError::Other(err.into()))?;

        Ok(())
    }

    fn load_from_file_contents(
        &self,
        settings: &crate::DataLoaderSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: Sender<crate::LoadedData>,
    ) -> Result<(), crate::DataLoaderError> {
        if !has_mcap_extension(&filepath) {
            return Err(DataLoaderError::Incompatible(filepath)); // simply not interested
        }

        re_tracing::profile_function!();

        let contents = contents.into_owned();
        let settings = settings.clone();
        let selected_layers = self.selected_layers.clone();
        let raw_fallback_enabled = self.raw_fallback_enabled;
        let lenses = self.lenses.clone();

        // NOTE: this must be spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                load_mcap(
                    &contents,
                    &settings,
                    &tx,
                    &selected_layers,
                    raw_fallback_enabled,
                    lenses.as_deref(),
                )?;
            } else {
                std::thread::Builder::new()
                    .name(format!("load_mcap({filepath:?})"))
                    .spawn(move || {
                        if let Err(err) = load_mcap(
                            &contents,
                            &settings,
                            &tx,
                            &selected_layers,
                            raw_fallback_enabled,
                            lenses.as_deref(),
                        ) {
                            re_log::error!("Failed to load MCAP file: {err}");
                        }
                    })
                    .map_err(|err| DataLoaderError::Other(err.into()))?;
            }
        }

        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_mcap_mmap(
    filepath: &std::path::PathBuf,
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
    selected_layers: &SelectedLayers,
    raw_fallback_enabled: bool,
    lenses: Option<&Lenses>,
) -> Result<(), DataLoaderError> {
    use std::fs::File;
    let file = File::open(filepath)?;

    // SAFETY: file-backed memory maps are marked unsafe because of potential UB when using the map and the underlying file is modified.
    #[expect(unsafe_code)]
    let mmap = unsafe { memmap2::Mmap::map(&file)? };

    load_mcap(
        &mmap,
        settings,
        tx,
        selected_layers,
        raw_fallback_enabled,
        lenses,
    )
}

pub fn load_mcap(
    mcap: &[u8],
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
    selected_layers: &SelectedLayers,
    raw_fallback_enabled: bool,
    lenses: Option<&Lenses>,
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

    let mut send_chunk = |chunk: re_chunk::Chunk| {
        // Apply lenses if configured, otherwise forward the chunk directly.
        if let Some(lenses) = lenses {
            for result in lenses.apply(&chunk) {
                match result {
                    Ok(transformed_chunk) => {
                        send_chunk_to_channel(tx, &store_id, transformed_chunk);
                    }
                    Err(partial_chunk) => {
                        for error in partial_chunk.errors() {
                            re_log::error_once!("Lens error: {error}");
                        }
                        if let Some(chunk) = partial_chunk.take() {
                            send_chunk_to_channel(tx, &store_id, chunk);
                        }
                    }
                }
            }
        } else {
            send_chunk_to_channel(tx, &store_id, chunk);
        }
    };

    let reader = Cursor::new(&mcap);

    let summary = re_mcap::read_summary(reader)?
        .ok_or_else(|| anyhow::anyhow!("MCAP file does not contain a summary"))?;

    // TODO(#10862): Add warning for channel that miss semantic information.
    LayerRegistry::all_builtin(raw_fallback_enabled)
        .select(selected_layers)
        .plan(&summary)?
        .run(mcap, &summary, &mut send_chunk)?;

    Ok(())
}

fn send_chunk_to_channel(tx: &Sender<LoadedData>, store_id: &StoreId, chunk: re_chunk::Chunk) {
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
}

fn store_info(store_id: StoreId) -> SetStoreInfo {
    SetStoreInfo {
        row_id: *RowId::new(),
        info: StoreInfo::new(
            store_id,
            re_log_types::StoreSource::Other(MCAP_LOADER_NAME.to_owned()),
        ),
    }
}

/// Checks if a path has the `.mcap` extension.
fn has_mcap_extension(filepath: &Path) -> bool {
    filepath
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("mcap"))
        .unwrap_or(false)
}
