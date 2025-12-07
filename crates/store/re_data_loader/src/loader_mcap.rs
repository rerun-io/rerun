//! Rerun dataloader for MCAP files.

use crate::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};
use lance_io::object_store::{ObjectStoreParams, ObjectStoreRegistry};
use mcap::sans_io::IndexedReaderOptions;
use re_chunk::RowId;
use re_log_types::{SetStoreInfo, StoreId, StoreInfo};
use re_mcap::{AsyncSeekRead, LayerRegistry, SelectedLayers};
use std::io::Cursor;
use std::path::Path;
use std::str::FromStr as _;

use std::sync::mpsc::Sender;
use url::Url;

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

fn mcap_option_from_url(url: &Url) -> anyhow::Result<IndexedReaderOptions> {
    let mut mcap_options = IndexedReaderOptions::default();
    for (name, val) in url.query_pairs() {
        if name == "start" {
            mcap_options = mcap_options.log_time_on_or_after(u64::from_str(&val)?);
        } else if name == "end" {
            mcap_options = mcap_options.log_time_before(u64::from_str(&val)?);
        }
    }

    Ok(mcap_options)
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
        re_tracing::profile_function!();
        let url = path.to_string_lossy();
        let mcap_url = match Url::parse(&url) {
            Ok(mcap_url) => mcap_url,
            Err(err) => match err {
                url::ParseError::RelativeUrlWithoutBase => {
                    match Url::parse(&format!("file://{url}")) {
                        Ok(mcap_url) => mcap_url,
                        Err(err) => return Err(DataLoaderError::Other(err.into())),
                    }
                }
                _ => return Err(DataLoaderError::Other(err.into())),
            },
        };

        let path = std::path::PathBuf::from_str(mcap_url.path()).unwrap();

        if mcap_url.scheme() == "file" && !is_mcap_file(&path) {
            return Err(DataLoaderError::Incompatible(path.clone())); // simply not interested
        }

        let mcap_options = match mcap_option_from_url(&mcap_url) {
            Ok(mcap_options) => mcap_options,
            Err(err) => return Err(DataLoaderError::Other(err)),
        };

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
                // Load from local disk.
                if mcap_url.scheme() == "file" {
                    if let Err(err) = load_mcap_mmap(
                        &path,
                        &mcap_options,
                        &settings,
                        &tx,
                        &selected_layers,
                        raw_fallback_enabled,
                    ) {
                        re_log::error!("Failed to load MCAP file: {err}");
                    }
                }
                // Load from cloud.
                else if let Err(err) = load_mcap_cloud(
                    &mcap_url,
                    &mcap_options,
                    &settings,
                    &tx,
                    &selected_layers,
                    raw_fallback_enabled,
                ) {
                    re_log::error!("Failed to load MCAP file: {err}");
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
                if let Err(err) = load_mcap_mmap(
                    &filepath,
                    &IndexedReaderOptions::default(),
                    &settings,
                    &tx,
                    &selected_layers,
                    raw_fallback_enabled,
                ) {
                    re_log::error!("Failed to load MCAP file: {err}");
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
        };
        let contents = contents.into_owned();
        load_mcap(
            Box::new(Cursor::new(contents)),
            &IndexedReaderOptions::default(),
            settings,
            &tx,
            &self.selected_layers,
            self.raw_fallback_enabled,
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_mcap_cloud(
    url: &Url,
    mcap_options: &IndexedReaderOptions,
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
    selected_layers: &SelectedLayers,
    raw_fallback_enabled: bool,
) -> anyhow::Result<()> {
    let mcap_reader = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let store = ObjectStoreRegistry::default()
                .get_store(url.clone(), &ObjectStoreParams::default())
                .await?;
            let meta = store
                .inner
                .head(&object_store::path::Path::parse(url.path())?)
                .await?;
            let mcap_reader = Box::from(object_store::buffered::BufReader::new(
                store.inner.clone(),
                &meta,
            ));

            Ok::<Box<object_store::buffered::BufReader>, anyhow::Error>(mcap_reader)
        })?;

    Ok(load_mcap(
        mcap_reader,
        mcap_options,
        settings,
        tx,
        selected_layers,
        raw_fallback_enabled,
    )?)
}

#[cfg(not(target_arch = "wasm32"))]
fn load_mcap_mmap(
    filepath: &std::path::PathBuf,
    mcap_options: &IndexedReaderOptions,
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
    selected_layers: &SelectedLayers,
    raw_fallback_enabled: bool,
) -> std::result::Result<(), DataLoaderError> {
    use std::fs::File;
    let file = File::open(filepath)?;

    // SAFETY: file-backed memory maps are marked unsafe because of potential UB when using the map and the underlying file is modified.
    #[expect(unsafe_code)]
    let mmap = unsafe { memmap2::Mmap::map(&file)? };

    load_mcap(
        Box::new(Cursor::new(mmap)),
        mcap_options,
        settings,
        tx,
        selected_layers,
        raw_fallback_enabled,
    )
}

pub fn load_mcap(
    mut mcap: Box<dyn AsyncSeekRead>,
    mcap_options: &IndexedReaderOptions,
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
    selected_layers: &SelectedLayers,
    raw_fallback_enabled: bool,
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

    let summary = re_mcap::read_summary(&mut mcap)?
        .ok_or_else(|| anyhow::anyhow!("MCAP file does not contain a summary"))?;

    // TODO(#10862): Add warning for channel that miss semantic information.
    LayerRegistry::all_builtin(raw_fallback_enabled)
        .select(selected_layers)
        .plan(&summary)?
        .run(&mut mcap, &summary, mcap_options, &mut send_chunk)?;

    Ok(())
}

pub fn store_info(store_id: StoreId) -> SetStoreInfo {
    SetStoreInfo {
        row_id: *RowId::new(),
        info: StoreInfo::new(
            store_id,
            re_log_types::StoreSource::Other(MCAP_LOADER_NAME.to_owned()),
        ),
    }
}

/// Checks if a file is an MCAP file.
fn is_mcap_file(filepath: &Path) -> bool {
    filepath.is_file()
        && filepath
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("mcap"))
            .unwrap_or(false)
}
