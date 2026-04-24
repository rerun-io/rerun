//! MCAP file importer implementation.

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

use crate::{ImportedData, Importer, ImporterError, ImporterSettings, URDF_DECODER_IDENTIFIER};

const MCAP_IMPORTER_NAME: &str = "McapImporter";

/// An [`Importer`] for MCAP files.
///
/// There are many different ways to extract and interpret information from MCAP files.
/// For example, it might be interesting to query for particular fields of messages,
/// or show information directly in the Rerun viewer. Because use-cases can vary, the
/// [`McapImporter`] is made up of [`re_mcap::Decoder`]s, each representing different views of the
/// underlying data.
///
/// These decoders can be specified in the CLI when converting an MCAP file
/// to an .rrd. Here are a few examples:
/// - [`re_mcap::decoders::McapProtobufDecoder`]
/// - [`re_mcap::decoders::McapRawDecoder`]
#[derive(Clone)]
pub struct McapImporter {
    selected_decoders: SelectedDecoders,
    // TODO(RR-3491): We don't need the fallback logic anymore; use `OutputMode` instead.
    raw_fallback_enabled: bool,
    lenses_by_time_type: HashMap<re_log_types::TimeType, Arc<Lenses>>,
}

impl Default for McapImporter {
    fn default() -> Self {
        Self::new(&SelectedDecoders::All)
    }
}

impl McapImporter {
    /// Creates a new [`McapImporter`] that uses the specified decoders.
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
        match super::lenses::mcap_lenses(selected_decoders, time_type) {
            Ok(Some(lenses)) => Some(Arc::new(lenses)),
            Ok(None) => None,
            Err(err) => {
                re_log::error_once!(
                    "Failed to build MCAP lenses: {err}. MCAP importer will run without them."
                );
                None
            }
        }
    }

    /// Load chunks from MCAP bytes, calling `emit_chunk` for each produced chunk.
    ///
    /// Bypasses the [`Importer`] / [`ImportedData`] / `SetStoreInfo` ceremony.
    /// Uses the decoders, raw fallback, and lenses already configured on this importer.
    pub fn emit_chunks(
        &self,
        mcap: &[u8],
        timeline_type: re_log_types::TimeType,
        timestamp_offset_ns: Option<i64>,
        emit_chunk: &mut dyn FnMut(re_chunk::Chunk),
    ) -> Result<(), ImporterError> {
        re_tracing::profile_function!();

        let lenses = self.lenses_for(timeline_type);

        let mut on_chunk_with_transforms = |chunk: re_chunk::Chunk| {
            if let Some(ref lenses) = lenses {
                for result in lenses.apply(&chunk) {
                    match result {
                        Ok(c) => emit_chunk(apply_timestamp_offset(c, timestamp_offset_ns)),
                        Err(partial) => {
                            for error in partial.errors() {
                                re_log::error_once!("Lens error: {error}");
                            }
                            if let Some(c) = partial.take() {
                                emit_chunk(apply_timestamp_offset(c, timestamp_offset_ns));
                            }
                        }
                    }
                }
            } else {
                emit_chunk(apply_timestamp_offset(chunk, timestamp_offset_ns));
            }
        };

        let reader = Cursor::new(&mcap);
        let summary = re_mcap::read_summary(reader)?
            .ok_or_else(|| anyhow::anyhow!("MCAP file does not contain a summary"))?;

        DecoderRegistry::all_builtin(self.raw_fallback_enabled)
            .select(&self.selected_decoders)
            .plan(mcap, &summary)?
            .run(mcap, &summary, timeline_type, &mut on_chunk_with_transforms)?;

        if self
            .selected_decoders
            .contains(&DecoderIdentifier::from(URDF_DECODER_IDENTIFIER))
            && let Err(err) = super::robot_description::extract_urdf_from_robot_descriptions(
                mcap,
                &summary,
                &mut on_chunk_with_transforms,
            )
        {
            re_log::warn_once!("Failed to extract URDF from robot_description topics: {err}");
        }

        Ok(())
    }
}

impl Importer for McapImporter {
    fn name(&self) -> crate::ImporterName {
        MCAP_IMPORTER_NAME.into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn import_from_path(
        &self,
        settings: &crate::ImporterSettings,
        path: std::path::PathBuf,
        tx: Sender<crate::ImportedData>,
    ) -> Result<(), ImporterError> {
        if !path.is_file() || !has_mcap_extension(&path) {
            return Err(ImporterError::Incompatible(path)); // simply not interested
        }

        re_tracing::profile_function!();

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of importers on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        let loader = self.clone();
        let settings = settings.clone();
        std::thread::Builder::new()
            .name(format!("load_mcap({path:?})"))
            .spawn(move || {
                let file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(err) => {
                        re_log::error!("Failed to open MCAP file: {err}");
                        return;
                    }
                };

                // SAFETY: file-backed mmap; we don't modify the file while mapped.
                #[expect(unsafe_code)]
                let mmap = match unsafe { memmap2::Mmap::map(&file) } {
                    Ok(m) => m,
                    Err(err) => {
                        re_log::error!("Failed to mmap MCAP file: {err}");
                        return;
                    }
                };

                if let Err(err) = loader.load_and_send(&mmap, &settings, &tx) {
                    re_log::error!("Failed to load MCAP file: {err}");
                }
            })
            .map_err(|err| ImporterError::Other(err.into()))?;

        Ok(())
    }

    fn import_from_file_contents(
        &self,
        settings: &crate::ImporterSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: Sender<crate::ImportedData>,
    ) -> Result<(), crate::ImporterError> {
        if !has_mcap_extension(&filepath) {
            return Err(ImporterError::Incompatible(filepath)); // simply not interested
        }

        re_tracing::profile_function!();

        let contents = contents.into_owned();
        let loader = self.clone();
        let settings = settings.clone();

        // NOTE: this must be spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of importers on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                loader.load_and_send(&contents, &settings, &tx)?;
            } else {
                std::thread::Builder::new()
                    .name(format!("load_mcap({filepath:?})"))
                    .spawn(move || {
                        if let Err(err) = loader.load_and_send(&contents, &settings, &tx) {
                            re_log::error!("Failed to load MCAP file: {err}");
                        }
                    })
                    .map_err(|err| ImporterError::Other(err.into()))?;
            }
        }

        Ok(())
    }
}

impl McapImporter {
    /// Send `SetStoreInfo` then decode chunks via [`Self::emit_chunks`],
    /// forwarding each chunk to the [`Importer`] channel.
    pub fn load_and_send(
        &self,
        mcap: &[u8],
        settings: &ImporterSettings,
        tx: &Sender<ImportedData>,
    ) -> Result<(), ImporterError> {
        re_log::debug!(
            "Loading MCAP with timeline type {:?}",
            settings.timeline_type
        );
        let store_id = settings.recommended_store_id();

        if send_crossbeam(
            tx,
            ImportedData::LogMsg(
                MCAP_IMPORTER_NAME.to_owned(),
                re_log_types::LogMsg::SetStoreInfo(store_info(store_id.clone())),
            ),
        )
        .is_err()
        {
            re_log::debug_once!(
                "Failed to send `SetStoreInfo` because smart channel closed unexpectedly."
            );
            return Ok(());
        }

        self.emit_chunks(
            mcap,
            settings.timeline_type,
            settings.timestamp_offset_ns,
            &mut |chunk| {
                send_chunk_to_channel(tx, &store_id, chunk);
            },
        )
    }
}

fn apply_timestamp_offset(mut chunk: re_chunk::Chunk, offset_ns: Option<i64>) -> re_chunk::Chunk {
    if let Some(offset_ns) = offset_ns {
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
    chunk
}

fn send_chunk_to_channel(tx: &Sender<ImportedData>, store_id: &StoreId, chunk: re_chunk::Chunk) {
    if send_crossbeam(
        tx,
        ImportedData::Chunk(MCAP_IMPORTER_NAME.to_owned(), store_id.clone(), chunk),
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
            re_log_types::StoreSource::Other(MCAP_IMPORTER_NAME.to_owned()),
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
