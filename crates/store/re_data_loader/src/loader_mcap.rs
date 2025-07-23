//! Rerun dataloader for MCAP files.

use std::{fs::File, io::Cursor, sync::mpsc::Sender};

use re_chunk::RowId;
use re_log_types::{ApplicationId, SetStoreInfo, StoreInfo};

use crate::mcap;
use crate::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};

pub struct McapLoader;

impl DataLoader for McapLoader {
    fn name(&self) -> crate::DataLoaderName {
        "McapLoader".into()
    }

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

        let file = File::open(&path)?;

        // SAFETY: file-backed memory maps are marked unsafe because of potential UB when using the map and the underlying file is modified.
        #[allow(unsafe_code)]
        let mmap = unsafe { memmap2::Mmap::map(&file)? };

        let settings = settings.clone();

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        std::thread::Builder::new()
            .name(format!("load_mcap({path:?}"))
            .spawn(move || match load_mcap(&mmap, &settings, &tx) {
                Ok(_) => {}
                Err(err) => {
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
    ) -> std::result::Result<(), crate::DataLoaderError> {
        if filepath.is_dir() || filepath.extension().is_none_or(|ext| ext != "mcap") {
            return Err(DataLoaderError::Incompatible(filepath)); // simply not interested
        }

        let settings = settings.clone();
        let contents = contents.into_owned();

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        std::thread::Builder::new()
            .name(format!("load_mcap({filepath:?}"))
            .spawn(move || match load_mcap(&contents, &settings, &tx) {
                Ok(_) => {}
                Err(err) => {
                    re_log::error!("Failed to load MCAP file: {err}");
                }
            })
            .map_err(|err| DataLoaderError::Other(err.into()))?;

        Ok(())
    }
}

fn load_mcap(
    mcap: &[u8],
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
) -> Result<(), DataLoaderError> {
    if tx
        .send(LoadedData::LogMsg(
            McapLoader.name(),
            re_log_types::LogMsg::SetStoreInfo(store_info(settings)),
        ))
        .is_err()
    {
        re_log::debug_once!(
            "Failed to send `SetStoreInfo` because smart channel closed unexpectedly."
        );
        // If the other side decided to hang up this is not our problem.
        return Ok(());
    }

    let reader = Cursor::new(&mcap);

    let summary = mcap::util::read_summary(reader)?
        .ok_or_else(|| anyhow::anyhow!("MCAP file does not contain a summary"))?;

    let properties_chunk = mcap::build_recording_properties_chunk(&summary)?;

    if tx
        .send(LoadedData::Chunk(
            McapLoader.name(),
            settings.store_id.clone(),
            properties_chunk,
        ))
        .is_err()
    {
        re_log::debug_once!(
            "Failed to send property chunk because the smart channel has been closed unexpectedly."
        );
        // If the other side decided to hang up this is not our problem.
        return Ok(());
    }

    let mut registry = mcap::decode::MessageDecoderRegistry::default();
    registry
        .register_default::<mcap::schema::sensor_msgs::ImuSchemaPlugin>()
        .register_default::<mcap::schema::sensor_msgs::ImageSchemaPlugin>()
        .register_default::<mcap::schema::sensor_msgs::CompressedImageSchemaPlugin>();

    for chunk in &summary.chunk_indexes {
        let channel_counts = mcap::util::get_chunk_message_count(chunk, &summary, mcap)?;

        re_log::trace!(
            "MCAP file contains {} channels with the following message counts: {:?}",
            channel_counts.len(),
            channel_counts
        );

        let mut decoder = mcap::decode::McapChunkDecoder::new(&registry, channel_counts);

        summary
            .stream_chunk(mcap, chunk)
            .map_err(|err| DataLoaderError::Other(err.into()))?
            .for_each(|msg| match msg {
                Ok(message) => {
                    if let Err(err) = decoder.decode_next(&message) {
                        re_log::error!(
                            "Failed to decode message from MCAP file: {err} on channel: {}",
                            message.channel.topic
                        );
                    }
                }
                Err(err) => {
                    re_log::error!("Failed to read message from MCAP file: {err}");
                }
            });

        for chunk in decoder.finish() {
            if let Ok(chunk) = chunk {
                if tx
                    .send(LoadedData::Chunk(
                        McapLoader.name(),
                        settings.store_id.clone(),
                        chunk,
                    ))
                    .is_err()
                {
                    re_log::debug_once!(
                        "Failed to send chunk, the smart channel has closed unexpectedly."
                    );
                    // If the other side decided to hang up this is not our problem.
                    break;
                }
            } else {
                re_log::error!("Failed to decode chunk from MCAP file: {:?}", chunk);
            }
        }
    }

    Ok(())
}

pub fn store_info(settings: &DataLoaderSettings) -> SetStoreInfo {
    let application_id = settings
        .application_id
        .clone()
        .unwrap_or(ApplicationId::random());

    SetStoreInfo {
        row_id: *RowId::new(),
        info: StoreInfo {
            application_id,
            store_id: settings.store_id.clone(),
            cloned_from: None,
            store_source: re_log_types::StoreSource::Other(McapLoader.name()),
            store_version: Some(re_build_info::CrateVersion::LOCAL),
        },
    }
}
