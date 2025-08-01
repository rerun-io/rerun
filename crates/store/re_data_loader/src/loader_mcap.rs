//! Rerun dataloader for MCAP files.

use std::sync::Arc;
use std::{io::Cursor, sync::mpsc::Sender};

use arrow::array::{
    MapBuilder, StringBuilder, UInt16Array, UInt16Builder, UInt32Array, UInt64Array, UInt64Builder,
};
use arrow::error::ArrowError;
use re_chunk::{Chunk, EntityPath, RowId, TimePoint};
use re_log_types::{SetStoreInfo, StoreId, StoreInfo};
use re_types::{AnyValues, archetypes, components};

use crate::mcap;
use crate::{DataLoader, DataLoaderError, DataLoaderSettings, LoadedData};

pub struct McapLoader;

impl DataLoader for McapLoader {
    fn name(&self) -> crate::DataLoaderName {
        "McapLoader".into()
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

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        let settings = settings.clone();
        std::thread::Builder::new()
            .name(format!("load_mcap({path:?}"))
            .spawn(move || match load_mcap_mmap(&path, &settings, &tx) {
                Ok(_) => {}
                Err(err) => {
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
        if filepath.is_dir() || filepath.extension().is_none_or(|ext| ext != "mcap") {
            return Err(DataLoaderError::Incompatible(filepath)); // simply not interested
        }

        let settings = settings.clone();

        // NOTE(1): `spawn` is fine, this whole function is native-only.
        // NOTE(2): this must spawned on a dedicated thread to avoid a deadlock!
        // `load` will spawn a bunch of loaders on the common rayon thread pool and wait for
        // their response via channels: we cannot be waiting for these responses on the
        // common rayon thread pool.
        std::thread::Builder::new()
            .name(format!("load_mcap({filepath:?}"))
            .spawn(move || match load_mcap_mmap(&filepath, &settings, &tx) {
                Ok(_) => {}
                Err(err) => {
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
        _filepath: std::path::PathBuf,
        contents: std::borrow::Cow<'_, [u8]>,
        tx: Sender<crate::LoadedData>,
    ) -> std::result::Result<(), DataLoaderError> {
        let contents = contents.into_owned();

        load_mcap(&contents, settings, &tx)
    }
}

fn from_channel(channel: &Arc<::mcap::Channel<'_>>) -> Result<AnyValues, ArrowError> {
    use arrow::array::{StringArray, UInt16Array};

    let ::mcap::Channel {
        id,
        topic,
        schema: _, // Separate archetype
        message_encoding,
        metadata,
    } = channel.as_ref();

    let key_builder = StringBuilder::new();
    let val_builder = StringBuilder::new();

    let mut builder = MapBuilder::new(None, key_builder, val_builder);

    for (key, val) in metadata {
        builder.keys().append_value(key);
        builder.values().append_value(val);
        builder.append(true)?;
    }

    let metadata = builder.finish();

    Ok(AnyValues::new("rerun.mcap.Channel")
        .with_field("id", Arc::new(UInt16Array::from(vec![*id])))
        .with_field("topic", Arc::new(StringArray::from(vec![topic.clone()])))
        .with_field("metadata", Arc::new(metadata))
        .with_field(
            "message_encoding",
            Arc::new(StringArray::from(vec![message_encoding.clone()])),
        ))
}

fn from_schema(schema: &Arc<::mcap::Schema<'_>>) -> AnyValues {
    use arrow::array::{StringArray, UInt16Array};

    let ::mcap::Schema {
        id,
        name,
        encoding,
        data,
    } = schema.as_ref();

    let blob = components::Blob(data.clone().into_owned().into());

    // Adds a field of arbitrary data to this archetype.
    AnyValues::new("rerun.mcap.Schema")
        .with_field("id", Arc::new(UInt16Array::from(vec![*id])))
        .with_field("name", Arc::new(StringArray::from(vec![name.clone()])))
        .with_component::<components::Blob>("data", vec![blob])
        .with_field(
            "encoding",
            Arc::new(StringArray::from(vec![encoding.clone()])),
        )
}

fn from_statistics(stats: &::mcap::records::Statistics) -> Result<AnyValues, ArrowError> {
    let ::mcap::records::Statistics {
        message_count,
        schema_count,
        channel_count,
        attachment_count,
        metadata_count,
        chunk_count,
        message_start_time,
        message_end_time,
        channel_message_counts,
    } = stats;

    let key_builder = UInt16Builder::new();
    let val_builder = UInt64Builder::new();

    let mut builder = MapBuilder::new(None, key_builder, val_builder);

    for (&key, &val) in channel_message_counts {
        builder.keys().append_value(key);
        builder.values().append_value(val);
        builder.append(true)?;
    }

    let channel_message_counts = builder.finish();

    Ok(AnyValues::new("rerun.mcap.Statistics")
        .with_field(
            "message_count",
            Arc::new(UInt64Array::from_value(*message_count, 1)),
        )
        .with_field(
            "schema_count",
            Arc::new(UInt16Array::from_value(*schema_count, 1)),
        )
        .with_field(
            "channel_count",
            Arc::new(UInt32Array::from_value(*channel_count, 1)),
        )
        .with_field(
            "attachment_count",
            Arc::new(UInt32Array::from_value(*attachment_count, 1)),
        )
        .with_field(
            "metadata_count",
            Arc::new(UInt32Array::from_value(*metadata_count, 1)),
        )
        .with_field(
            "chunk_count",
            Arc::new(UInt32Array::from_value(*chunk_count, 1)),
        )
        .with_component::<components::Timestamp>(
            "message_start_time",
            vec![*message_start_time as i64],
        )
        .with_component::<components::Timestamp>("message_end_time", vec![*message_end_time as i64])
        .with_field("channel_message_counts", Arc::new(channel_message_counts)))
}

#[cfg(not(target_arch = "wasm32"))]
fn load_mcap_mmap(
    filepath: &std::path::PathBuf,
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
) -> std::result::Result<(), DataLoaderError> {
    use std::fs::File;
    let file = File::open(filepath)?;

    // SAFETY: file-backed memory maps are marked unsafe because of potential UB when using the map and the underlying file is modified.
    #[allow(unsafe_code)]
    let mmap = unsafe { memmap2::Mmap::map(&file)? };

    load_mcap(&mmap, settings, tx)
}

fn load_mcap(
    mcap: &[u8],
    settings: &DataLoaderSettings,
    tx: &Sender<LoadedData>,
) -> Result<(), DataLoaderError> {
    let store_id = settings.recommended_store_id();

    if tx
        .send(LoadedData::LogMsg(
            McapLoader.name(),
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

    let send_chunk = |chunk| {
        if tx
            .send(LoadedData::Chunk(
                McapLoader.name(),
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

    let properties_chunk = mcap::build_recording_properties_chunk(&summary)?;
    send_chunk(properties_chunk);

    let mut registry = mcap::decode::MessageDecoderRegistry::default();
    registry
        .register_default::<mcap::schema::sensor_msgs::CameraInfoSchemaPlugin>()
        .register_default::<mcap::schema::sensor_msgs::CompressedImageSchemaPlugin>()
        .register_default::<mcap::schema::sensor_msgs::ImageSchemaPlugin>()
        .register_default::<mcap::schema::sensor_msgs::ImuSchemaPlugin>()
        .register_default::<mcap::schema::sensor_msgs::PointCloud2SchemaPlugin>()
        .register_default::<mcap::schema::std_msgs::StringSchemaPlugin>();

    // Send warnings for unsupported messages.
    for channel in summary.channels.values() {
        if let Some(schema) = channel.schema.as_ref() {
            if !registry.has_schema(&schema.name) {
                let chunk = Chunk::builder(EntityPath::from(channel.topic.clone()))
                    .with_archetype(
                        RowId::new(),
                        TimePoint::STATIC,
                        &archetypes::TextLog::new("Unsupported schema for channel")
                            .with_level(components::TextLogLevel::WARN),
                    )
                    .build()?;
                send_chunk(chunk);
            }
        } else {
            let chunk = Chunk::builder(EntityPath::from(channel.topic.clone()))
                .with_archetype(
                    RowId::new(),
                    TimePoint::STATIC,
                    &archetypes::TextLog::new("Missing schema for channel")
                        .with_level(components::TextLogLevel::ERROR),
                )
                .build()?;
            send_chunk(chunk);
        }
    }

    // Send static channel and schema information.
    for channel in summary.channels.values() {
        let chunk = Chunk::builder(channel.topic.as_str())
            .with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &[
                    from_channel(channel)?,
                    channel.schema.as_ref().map(from_schema).unwrap_or_default(),
                ],
            )
            .build()?;
        send_chunk(chunk);
    }

    // Send the statistics as recording properties.
    if let Some(statistics) = summary.stats.as_ref() {
        let chunk = Chunk::builder(EntityPath::properties())
            .with_archetype(
                RowId::new(),
                TimePoint::STATIC,
                &from_statistics(statistics)?,
            )
            .build()?;
        send_chunk(chunk);
    }

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
                        store_id.clone(),
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

pub fn store_info(store_id: StoreId) -> SetStoreInfo {
    SetStoreInfo {
        row_id: *RowId::new(),
        info: StoreInfo {
            store_id,
            cloned_from: None,
            store_source: re_log_types::StoreSource::Other(McapLoader.name()),
            store_version: Some(re_build_info::CrateVersion::LOCAL),
        },
    }
}
