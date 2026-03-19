//! MCAP file loader implementation.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use crossbeam::channel::Sender;
use re_chunk::RowId;
use re_lenses::Lenses;
use re_log_types::{SetStoreInfo, StoreId, StoreInfo};
use re_mcap::{DecoderIdentifier, DecoderRegistry, SelectedDecoders};
use re_quota_channel::send_crossbeam;

use crate::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};

const MCAP_LOADER_NAME: &str = "McapLoader";

/// A [`DataLoader`] for MCAP files.
///
/// There are many different ways to extract and interpret information from MCAP files.
/// For example, it might be interesting to query for particular fields of messages,
/// or show information directly in the Rerun viewer. Because use-cases can vary, the
/// [`McapLoader`] is made up of [`re_mcap::Decoder`]s, each representing different views of the
/// underlying data.
///
/// These decoders can be specified in the CLI when converting an MCAP file
/// to an .rrd. Here are a few examples:
/// - [`re_mcap::decoders::McapProtobufDecoder`]
/// - [`re_mcap::decoders::McapRawDecoder`]
pub struct McapLoader {
    selected_decoders: SelectedDecoders,
    // TODO(RR-3491): We don't need the fallback logic anymore; use `OutputMode` instead.
    raw_fallback_enabled: bool,
    lenses_by_time_type: HashMap<re_log_types::TimeType, Arc<Lenses>>,
}

impl Default for McapLoader {
    fn default() -> Self {
        Self::new(&SelectedDecoders::All)
    }
}

impl McapLoader {
    /// Creates a new [`McapLoader`] that uses the specified decoders.
    pub fn new(selected_decoders: &SelectedDecoders) -> Self {
        // Cache lenses for each supported timeline type.
        let mut lenses_by_time_type = HashMap::new();
        for time_type in [
            re_log_types::TimeType::TimestampNs,
            re_log_types::TimeType::DurationNs,
        ] {
            if let Some(lenses) = Self::build_lenses(selected_decoders, time_type) {
                lenses_by_time_type.insert(time_type, lenses);
            }
        }
        Self {
            selected_decoders: selected_decoders.clone(),
            raw_fallback_enabled: true,
            lenses_by_time_type,
        }
    }

    /// Configures whether the raw decoder is used as a fallback for unsupported channels.
    pub fn with_raw_fallback(mut self, raw_fallback_enabled: bool) -> Self {
        self.raw_fallback_enabled = raw_fallback_enabled;
        self
    }

    /// Returns the cached lenses for the given [`re_log_types::TimeType`].
    fn lenses_for(&self, time_type: re_log_types::TimeType) -> Option<Arc<Lenses>> {
        if time_type == re_log_types::TimeType::Sequence {
            re_log::error_once!("Sequence is not a supported timeline type for MCAP lenses");
            return None;
        }
        self.lenses_by_time_type.get(&time_type).cloned()
    }

    fn build_lenses(
        selected_decoders: &SelectedDecoders,
        time_type: re_log_types::TimeType,
    ) -> Option<Arc<Lenses>> {
        if !selected_decoders.contains(&DecoderIdentifier::from(
            super::lenses::FOXGLOVE_LENSES_IDENTIFIER,
        )) {
            return None;
        }

        match super::lenses::foxglove_lenses(time_type) {
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
        let selected_decoders = self.selected_decoders.clone();
        let raw_fallback_enabled = self.raw_fallback_enabled;
        let lenses = self.lenses_for(settings.timeline_type);
        std::thread::Builder::new()
            .name(format!("load_mcap({path:?}"))
            .spawn(move || {
                if let Err(err) = load_mcap_mmap(
                    &path,
                    &settings,
                    &tx,
                    &selected_decoders,
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
        let selected_decoders = self.selected_decoders.clone();
        let raw_fallback_enabled = self.raw_fallback_enabled;
        let lenses = self.lenses_for(settings.timeline_type);

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
                    &selected_decoders,
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
                            &selected_decoders,
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
    selected_decoders: &SelectedDecoders,
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
        selected_decoders,
        raw_fallback_enabled,
        lenses,
    )
}

pub fn load_mcap(
    mcap: &[u8],
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
    selected_decoders: &SelectedDecoders,
    raw_fallback_enabled: bool,
    lenses: Option<&Lenses>,
) -> Result<(), DataLoaderError> {
    re_tracing::profile_function!();
    re_log::debug!(
        "Loading MCAP with timeline type {:?}",
        settings.timeline_type
    );
    let store_id = settings.recommended_store_id();

    if send_crossbeam(
        tx,
        LoadedData::LogMsg(
            MCAP_LOADER_NAME.to_owned(),
            re_log_types::LogMsg::SetStoreInfo(store_info(store_id.clone())),
        ),
    )
    .is_err()
    {
        re_log::debug_once!(
            "Failed to send `SetStoreInfo` because smart channel closed unexpectedly."
        );
        // If the other side decided to hang up this is not our problem.
        return Ok(());
    }

    let timestamp_offset_ns = settings.timestamp_offset_ns;
    let time_type = settings.timeline_type;

    let mut send_chunk = |chunk: re_chunk::Chunk| {
        // Apply lenses if configured, otherwise forward the chunk directly.
        if let Some(lenses) = lenses {
            for result in lenses.apply(&chunk) {
                match result {
                    Ok(transformed_chunk) => {
                        send_chunk_to_channel(
                            tx,
                            &store_id,
                            transformed_chunk,
                            timestamp_offset_ns,
                        );
                    }
                    Err(partial_chunk) => {
                        for error in partial_chunk.errors() {
                            re_log::error_once!("Lens error: {error}");
                        }
                        if let Some(chunk) = partial_chunk.take() {
                            send_chunk_to_channel(tx, &store_id, chunk, timestamp_offset_ns);
                        }
                    }
                }
            }
        } else {
            send_chunk_to_channel(tx, &store_id, chunk, timestamp_offset_ns);
        }
    };

    let reader = Cursor::new(&mcap);

    let summary = re_mcap::read_summary(reader)?
        .ok_or_else(|| anyhow::anyhow!("MCAP file does not contain a summary"))?;

    // TODO(#10862): Add warning for channel that miss semantic information.
    DecoderRegistry::all_builtin(raw_fallback_enabled)
        .select(selected_decoders)
        .plan(mcap, &summary)?
        .run(mcap, &summary, time_type, &mut send_chunk)?;

    // Extract URDF from robot_description topics and convert to 3D visualization chunks.
    // Non-fatal: errors here should never prevent the rest of the MCAP from loading.
    // TODO(michael): make the URDF extraction a proper decoder.
    if selected_decoders.contains(&DecoderIdentifier::from("urdf"))
        && let Err(err) = super::robot_description::extract_urdf_from_robot_descriptions(
            mcap,
            &summary,
            &mut send_chunk,
        )
    {
        re_log::warn_once!("Failed to extract URDF from robot_description topics: {err}");
    }

    Ok(())
}

fn send_chunk_to_channel(
    tx: &Sender<LoadedData>,
    store_id: &StoreId,
    mut chunk: re_chunk::Chunk,
    timestamp_offset_ns: Option<i64>,
) {
    if let Some(offset_ns) = timestamp_offset_ns {
        let offset_timelines: Vec<_> = chunk
            .timelines()
            .values()
            .filter(|time_col| time_col.timeline().typ() == re_log_types::TimeType::TimestampNs)
            .map(|time_col| time_col.offset_by_nanos(offset_ns))
            .collect();
        for time_col in offset_timelines {
            chunk.add_timeline(time_col).ok();
        }
    }

    if send_crossbeam(
        tx,
        LoadedData::Chunk(MCAP_LOADER_NAME.to_owned(), store_id.clone(), chunk),
    )
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
