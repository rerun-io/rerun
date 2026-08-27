use re_log_encoding::Decoder;
use re_log_types::ApplicationId;

use crate::{ImportedData, Importer as _};

// ---

/// Imports data from any `rrd` file or in-memory contents.
pub struct RrdImporter;

impl crate::Importer for RrdImporter {
    #[inline]
    fn name(&self) -> String {
        "rerun.importers.Rrd".into()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn import_from_path(
        &self,
        settings: &crate::ImporterSettings,
        filepath: std::path::PathBuf,
        tx: crossbeam::channel::Sender<crate::ImportedData>,
    ) -> Result<(), crate::ImporterError> {
        use anyhow::Context as _;

        re_tracing::profile_function!(filepath.display().to_string());

        let mut extension = crate::extension(&filepath);
        if !matches!(extension.as_str(), "rbl" | "rrd") {
            if filepath.is_file() || filepath.is_dir() {
                // NOTE: blueprints and recordings have the same file format
                return Err(crate::ImporterError::Incompatible(filepath.clone()));
            }
            // NOTE(1): If this is some kind of virtual file (fifo, socket, pipe, etc), then we
            // always assume it's an RRD stream by default.
            //
            // NOTE(2): Because waiting for an end-of-stream marker on a pipe doesn't make sense,
            // we tag it as `rbl` instead of `rrd` (but really this just means: please don't block
            // indefinitely).
            extension = "rbl".to_owned();
        }

        re_log::debug!(
            ?filepath,
            importer = self.name(),
            "Loading rrd data from filesystem…",
        );

        match extension.as_str() {
            "rbl" => {
                // We assume .rbl is not streamed and no retrying after seeing EOF is needed.
                // Otherwise we'd risk retrying to read .rbl file that has no end-of-stream header and
                // blocking the UI update thread indefinitely and making the viewer unresponsive (as .rbl
                // files are sometimes read on UI update).
                let file = std::fs::File::open(&filepath)
                    .with_context(|| format!("Failed to open file {filepath:?}"))?;
                let file = std::io::BufReader::new(file);

                let messages = Decoder::decode_eager(file)?;

                // NOTE: This is IO bound, it must run on a dedicated thread, not the shared rayon thread pool.
                std::thread::Builder::new()
                    .name(format!("decode_and_stream({filepath:?})"))
                    .spawn({
                        let filepath = filepath.clone();
                        let settings = settings.clone();
                        move || {
                            decode_and_stream(
                                &filepath,
                                &tx,
                                messages,
                                settings
                                    .opened_store_id
                                    .as_ref()
                                    .map(|store_id| store_id.application_id()),
                                // We never want to patch blueprints' store IDs, only their app IDs.
                                None,
                            );
                        }
                    })
                    .with_context(|| format!("Failed to spawn IO thread for {filepath:?}"))?;
            }

            "rrd" => {
                let file = std::fs::File::open(&filepath)
                    .with_context(|| format!("Failed to open file {filepath:?}"))?;
                let file = std::io::BufReader::new(file);

                let messages = Decoder::decode_eager(file)?;

                // NOTE: This is IO bound, it must run on a dedicated thread, not the shared rayon thread pool.
                std::thread::Builder::new()
                    .name(format!("decode_and_stream({filepath:?})"))
                    .spawn({
                        let filepath = filepath.clone();
                        move || {
                            decode_and_stream(
                                &filepath, &tx, messages,
                                // Never use import semantics for .rrd files
                                None, None,
                            );
                        }
                    })
                    .with_context(|| format!("Failed to spawn IO thread for {filepath:?}"))?;
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    fn import_from_file_contents(
        &self,
        settings: &crate::ImporterSettings,
        filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: crossbeam::channel::Sender<crate::ImportedData>,
    ) -> Result<(), crate::ImporterError> {
        re_tracing::profile_function!(filepath.display().to_string());

        let extension = crate::extension(&filepath);
        if !matches!(extension.as_str(), "rbl" | "rrd") {
            // NOTE: blueprints and recordings has the same file format
            return Err(crate::ImporterError::Incompatible(filepath));
        }

        let contents = std::io::Cursor::new(contents);
        let messages = match Decoder::decode_eager(contents) {
            Ok(decoder) => decoder,
            Err(err) => match err {
                // simply not interested
                re_log_encoding::DecodeError::Codec(
                    re_log_encoding::rrd::CodecError::NotAnRrd(_)
                    | re_log_encoding::rrd::CodecError::InvalidOptions(_),
                ) => return Ok(()),
                _ => return Err(err.into()),
            },
        };

        // * We never want to patch blueprints' store IDs, only their app IDs.
        // * We never use import semantics at all for .rrd files.
        let forced_application_id = if extension == "rbl" {
            settings
                .opened_store_id
                .as_ref()
                .map(|store_id| store_id.application_id())
        } else {
            None
        };
        let forced_recording_id = None;

        decode_and_stream(
            &filepath,
            &tx,
            messages,
            forced_application_id,
            forced_recording_id,
        );

        Ok(())
    }
}

fn decode_and_stream(
    filepath: &std::path::Path,
    tx: &crossbeam::channel::Sender<crate::ImportedData>,
    msgs: impl Iterator<Item = Result<re_log_types::LogMsg, re_log_encoding::DecodeError>>,
    forced_application_id: Option<&ApplicationId>,
    forced_recording_id: Option<&String>,
) {
    re_tracing::profile_function!(filepath.display().to_string());

    for msg in msgs {
        let msg = match msg {
            Ok(msg) => msg,
            Err(err) => {
                re_log::warn!(?filepath, "Failed to decode message: {err}");
                continue;
            }
        };

        let msg = if forced_application_id.is_some() || forced_recording_id.is_some() {
            match msg {
                re_log_types::LogMsg::SetStoreInfo(set_store_info) => {
                    let mut store_id = set_store_info.info.store_id.clone();
                    if let Some(forced_application_id) = forced_application_id {
                        store_id = store_id.with_application_id(forced_application_id.clone());
                    }
                    if let Some(forced_recording_id) = forced_recording_id {
                        store_id = store_id.with_recording_id(forced_recording_id.clone());
                    }

                    re_log_types::LogMsg::SetStoreInfo(re_log_types::SetStoreInfo {
                        info: re_log_types::StoreInfo {
                            store_id,
                            ..set_store_info.info
                        },
                        ..set_store_info
                    })
                }

                re_log_types::LogMsg::ArrowMsg(mut store_id, arrow_msg) => {
                    if let Some(forced_application_id) = forced_application_id {
                        store_id = store_id.with_application_id(forced_application_id.clone());
                    }
                    if let Some(forced_recording_id) = forced_recording_id {
                        store_id = store_id.with_recording_id(forced_recording_id.clone());
                    }

                    re_log_types::LogMsg::ArrowMsg(store_id, arrow_msg)
                }

                re_log_types::LogMsg::BlueprintActivationCommand(blueprint_activation_command) => {
                    let mut blueprint_id = blueprint_activation_command.blueprint_id.clone();
                    if let Some(forced_application_id) = forced_application_id {
                        blueprint_id =
                            blueprint_id.with_application_id(forced_application_id.clone());
                    }
                    re_log_types::LogMsg::BlueprintActivationCommand(
                        re_log_types::BlueprintActivationCommand {
                            blueprint_id,
                            ..blueprint_activation_command
                        },
                    )
                }
            }
        } else {
            msg
        };

        let data = ImportedData::LogMsg(RrdImporter::name(&RrdImporter), msg);
        if re_quota_channel::send_crossbeam(tx, data).is_err() {
            break; // The other end has decided to hang up, not our problem.
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, path::PathBuf};

    use re_chunk::RowId;
    use re_log_encoding::Encoder;
    use re_log_types::{
        BlueprintActivationCommand, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind,
        StoreSource, TimePoint,
    };
    use re_sdk_types::archetypes::TextDocument;

    use super::*;

    fn text_chunk(entity_path: &str, text: &str) -> re_chunk::Chunk {
        re_chunk::Chunk::builder(entity_path)
            .with_archetype(RowId::new(), TimePoint::STATIC, &TextDocument::new(text))
            .build()
            .expect("chunk should build")
    }

    fn encode(msgs: impl IntoIterator<Item = LogMsg>) -> Vec<u8> {
        Encoder::encode(msgs.into_iter().map(Ok)).expect("messages should encode")
    }

    /// Runs [`RrdImporter`] over `encoded` and collects the log messages it emits.
    fn import(opened_store_id: &StoreId, filename: &str, encoded: &[u8]) -> Vec<LogMsg> {
        let mut settings = crate::ImporterSettings::recommended("opened_recording");
        settings.opened_store_id = Some(opened_store_id.clone());

        // The importer and receiver run on this test thread, so the queue cannot grow independently.
        #[expect(
            clippy::disallowed_methods,
            reason = "the sender and receiver are on the same thread"
        )]
        let (tx, rx) = crossbeam::channel::unbounded();

        RrdImporter
            .import_from_file_contents(
                &settings,
                PathBuf::from(filename),
                Cow::Borrowed(encoded),
                tx,
            )
            .expect("import should succeed");

        rx.into_iter()
            .map(|data| match data {
                ImportedData::LogMsg(_, msg) => msg,
                other => panic!("rrd import should only produce log messages, got {other:?}"),
            })
            .collect()
    }

    #[test]
    fn importing_blueprint_retargets_all_messages_to_opened_application() {
        let source_blueprint_id =
            StoreId::random(StoreKind::Blueprint, "unrelated_blueprint_application");
        let activation = BlueprintActivationCommand::make_active(source_blueprint_id.clone());
        let encoded = encode([
            LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::ZERO,
                info: StoreInfo::new(source_blueprint_id.clone(), StoreSource::Unknown),
            }),
            LogMsg::ArrowMsg(
                source_blueprint_id.clone(),
                text_chunk("blueprint/entity", "blueprint data")
                    .to_arrow_msg()
                    .expect("blueprint chunk should encode"),
            ),
            LogMsg::BlueprintActivationCommand(activation.clone()),
        ]);

        let opened_store_id = StoreId::random(StoreKind::Recording, "opened_application");
        let imported = import(&opened_store_id, "imported_blueprint.rbl", &encoded);

        let [
            LogMsg::SetStoreInfo(imported_store_info),
            LogMsg::ArrowMsg(imported_chunk_store_id, imported_arrow_msg),
            LogMsg::BlueprintActivationCommand(imported_activation),
        ] = imported.as_slice()
        else {
            panic!("blueprint import should preserve its three-message stream");
        };

        for imported_store_id in [
            &imported_store_info.info.store_id,
            imported_chunk_store_id,
            &imported_activation.blueprint_id,
        ] {
            assert_eq!(
                imported_store_id.application_id(),
                opened_store_id.application_id()
            );
            assert_eq!(
                imported_store_id.recording_id(),
                source_blueprint_id.recording_id()
            );
            assert!(imported_store_id.is_blueprint());
        }
        let imported_chunk = re_chunk::Chunk::from_arrow_msg(imported_arrow_msg)
            .expect("imported blueprint chunk should decode");
        assert_eq!(
            imported_chunk.entity_path().to_string(),
            "/blueprint/entity"
        );
        assert_eq!(imported_activation.make_active, activation.make_active);
        assert_eq!(imported_activation.make_default, activation.make_default);
    }

    /// Counterpart to the test above: only `.rbl` files adopt the opened application's ID.
    /// Importing an `.rrd` next to an open recording must leave every store ID alone, or its
    /// contents would be reassigned to whichever application happens to be open.
    #[test]
    fn importing_recording_leaves_all_store_ids_alone() {
        let source_recording_id =
            StoreId::random(StoreKind::Recording, "unrelated_recording_application");
        let source_blueprint_id =
            StoreId::random(StoreKind::Blueprint, "unrelated_recording_application");
        let encoded = encode([
            LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::ZERO,
                info: StoreInfo::new(source_recording_id.clone(), StoreSource::Unknown),
            }),
            LogMsg::ArrowMsg(
                source_recording_id.clone(),
                text_chunk("recording/entity", "recording data")
                    .to_arrow_msg()
                    .expect("recording chunk should encode"),
            ),
            LogMsg::SetStoreInfo(SetStoreInfo {
                row_id: *RowId::ZERO,
                info: StoreInfo::new(source_blueprint_id.clone(), StoreSource::Unknown),
            }),
            LogMsg::BlueprintActivationCommand(BlueprintActivationCommand::make_active(
                source_blueprint_id.clone(),
            )),
        ]);

        let opened_store_id = StoreId::random(StoreKind::Recording, "opened_application");
        let imported = import(&opened_store_id, "imported_recording.rrd", &encoded);

        let [
            LogMsg::SetStoreInfo(imported_recording_info),
            LogMsg::ArrowMsg(imported_chunk_store_id, _),
            LogMsg::SetStoreInfo(imported_blueprint_info),
            LogMsg::BlueprintActivationCommand(imported_activation),
        ] = imported.as_slice()
        else {
            panic!("recording import should preserve its four-message stream");
        };

        for imported_store_id in [
            &imported_recording_info.info.store_id,
            imported_chunk_store_id,
        ] {
            assert_eq!(imported_store_id, &source_recording_id);
        }
        for imported_store_id in [
            &imported_blueprint_info.info.store_id,
            &imported_activation.blueprint_id,
        ] {
            assert_eq!(imported_store_id, &source_blueprint_id);
        }
    }
}
