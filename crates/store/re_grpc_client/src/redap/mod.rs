use arrow::array::RecordBatch as ArrowRecordBatch;
use tokio_stream::StreamExt as _;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::Chunk;
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::{
    ApplicationId, LogMsg, SetStoreInfo, StoreId, StoreInfo, StoreKind, StoreSource, Time,
};
use re_protos::{
    common::v0::RecordingId,
    remote_store::v0::{
        CatalogFilter, FetchRecordingRequest, QueryCatalogRequest, CATALOG_APP_ID_FIELD_NAME,
        CATALOG_START_TIME_FIELD_NAME,
    },
};

// ----------------------------------------------------------------------------

mod address;

pub use address::{ConnectionError, Origin, RedapAddress};

use crate::spawn_future;
use crate::StreamError;
use crate::TonicStatusError;

// ----------------------------------------------------------------------------

/// Stream an rrd file or metadata catalog over gRPC from a Rerun Data Platform server.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_from_redap(
    url: String,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<re_smart_channel::Receiver<LogMsg>, ConnectionError> {
    re_log::debug!("Loading {url}…");

    let address = url.as_str().try_into()?;

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RerunGrpcStream { url: url.clone() },
        re_smart_channel::SmartChannelSource::RerunGrpcStream { url: url.clone() },
    );

    match address {
        RedapAddress::Recording {
            origin,
            recording_id,
        } => {
            spawn_future(async move {
                if let Err(err) = stream_recording_async(tx, origin, recording_id, on_msg).await {
                    re_log::error!(
                        "Error while streaming {url}: {}",
                        re_error::format_ref(&err)
                    );
                }
            });
        }
        // TODO(#9058): This should be fix by introducing a `RedapRecordingAddress`.
        RedapAddress::Catalog { origin } => {
            return Err(ConnectionError::CannotLoadURLAsRecording {
                url: origin.to_string(),
            });
        }
    }

    Ok(rx)
}

pub async fn stream_recording_async(
    tx: re_smart_channel::Sender<LogMsg>,
    origin: Origin,
    recording_id: String,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    re_log::debug!("Connecting to {origin}…");
    let mut client = origin.client().await?;

    re_log::debug!("Fetching catalog data for {recording_id}…");

    let resp = client
        .query_catalog(QueryCatalogRequest {
            column_projection: None, // fetch all columns
            filter: Some(CatalogFilter {
                recording_ids: vec![RecordingId {
                    id: recording_id.clone(),
                }],
            }),
        })
        .await
        .map_err(TonicStatusError)?
        .into_inner()
        .map(|resp| {
            resp.and_then(|r| {
                r.decode()
                    .map_err(|err| tonic::Status::internal(err.to_string()))
            })
        })
        .collect::<Result<Vec<_>, tonic::Status>>()
        .await
        .map_err(TonicStatusError)?;

    if resp.len() != 1 || resp[0].num_rows() != 1 {
        return Err(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: format!(
                "expected exactly one recording with id {recording_id}, got {}",
                resp.len()
            ),
        }));
    }

    let store_info = store_info_from_catalog_chunk(&resp[0].clone(), &recording_id)?;
    let store_id = store_info.store_id.clone();

    re_log::debug!("Fetching {recording_id}…");

    let mut resp = client
        .fetch_recording(FetchRecordingRequest {
            recording_id: Some(RecordingId {
                id: recording_id.clone(),
            }),
        })
        .await
        .map_err(TonicStatusError)?
        .into_inner()
        .map(|resp| {
            resp.and_then(|r| {
                r.decode()
                    .map_err(|err| tonic::Status::internal(err.to_string()))
            })
        });

    drop(client);

    // We need a whole StoreInfo here.
    if tx
        .send(LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *re_chunk::RowId::new(),
            info: store_info,
        }))
        .is_err()
    {
        re_log::debug!("Receiver disconnected");
        return Ok(());
    }

    re_log::info!("Starting to read...");
    while let Some(result) = resp.next().await {
        let batch = result.map_err(TonicStatusError)?;
        let chunk = Chunk::from_record_batch(&batch)?;

        if tx
            .send(LogMsg::ArrowMsg(store_id.clone(), chunk.to_arrow_msg()?))
            .is_err()
        {
            re_log::debug!("Receiver disconnected");
            return Ok(());
        }

        if let Some(on_msg) = &on_msg {
            on_msg();
        }
    }

    Ok(())
}

pub fn store_info_from_catalog_chunk(
    record_batch: &ArrowRecordBatch,
    recording_id: &str,
) -> Result<StoreInfo, StreamError> {
    let store_id = StoreId::from_string(StoreKind::Recording, recording_id.to_owned());

    let data = record_batch
        .column_by_name(CATALOG_APP_ID_FIELD_NAME)
        .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: format!("no {CATALOG_APP_ID_FIELD_NAME} field found"),
        }))?;
    let app_id = data
        .downcast_array_ref::<arrow::array::StringArray>()
        .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: format!(
                "{CATALOG_APP_ID_FIELD_NAME} must be a utf8 array: {:?}",
                record_batch.schema_ref()
            ),
        }))?
        .value(0);

    let data = record_batch
        .column_by_name(CATALOG_START_TIME_FIELD_NAME)
        .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: format!("no {CATALOG_START_TIME_FIELD_NAME} field found"),
        }))?;
    let start_time = data
        .downcast_array_ref::<arrow::array::TimestampNanosecondArray>()
        .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: format!(
                "{CATALOG_START_TIME_FIELD_NAME} must be a Timestamp array: {:?}",
                record_batch.schema_ref()
            ),
        }))?
        .value(0);

    Ok(StoreInfo {
        application_id: ApplicationId::from(app_id),
        store_id: store_id.clone(),
        cloned_from: None,
        is_official_example: false,
        started: Time::from_ns_since_epoch(start_time),
        store_source: StoreSource::Unknown,
        store_version: None,
    })
}
