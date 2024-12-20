//! Communications with an Rerun Data Platform gRPC server.

mod address;

pub use address::{InvalidRedapAddress, RedapAddress};
use re_chunk::external::arrow2;
use re_log_types::external::re_types_core::ComponentDescriptor;
use re_protos::remote_store::v0::CatalogFilter;
use re_types::blueprint::archetypes::{ContainerBlueprint, ViewportBlueprint};
use re_types::blueprint::archetypes::{ViewBlueprint, ViewContents};
use re_types::blueprint::components::{ContainerKind, RootContainer};
use re_types::components::RecordingUri;
use re_types::external::uuid;
use re_types::{Archetype, Component};
use url::Url;

// ----------------------------------------------------------------------------

use std::error::Error;
use std::sync::Arc;

use arrow2::array::Utf8Array as Arrow2Utf8Array;
use arrow2::datatypes::Field as Arrow2Field;
use re_chunk::{
    Arrow2Array, Chunk, ChunkBuilder, ChunkId, EntityPath, RowId, Timeline, TransportChunk,
};
use re_log_encoding::codec::{wire::decode, CodecError};
use re_log_types::{
    ApplicationId, BlueprintActivationCommand, EntityPathFilter, LogMsg, SetStoreInfo, StoreId,
    StoreInfo, StoreKind, StoreSource, Time,
};
use re_protos::common::v0::RecordingId;
use re_protos::remote_store::v0::{
    storage_node_client::StorageNodeClient, FetchRecordingRequest, QueryCatalogRequest,
};

// ----------------------------------------------------------------------------

/// Wrapper with a nicer error message
#[derive(Debug)]
pub struct TonicStatusError(pub tonic::Status);

impl std::fmt::Display for TonicStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = &self.0;
        write!(f, "gRPC error, status: '{}'", status.code())?;
        if !status.message().is_empty() {
            write!(f, ", message: {:?}", status.message())?;
        }
        // Binary data - not useful.
        // if !status.details().is_empty() {
        //     write!(f, ", details: {:?}", status.details())?;
        // }
        if !status.metadata().is_empty() {
            write!(f, ", metadata: {:?}", status.metadata())?;
        }
        Ok(())
    }
}

impl Error for TonicStatusError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StreamError {
    /// Native connection error
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Transport(#[from] tonic::transport::Error),

    #[error(transparent)]
    TonicStatus(#[from] TonicStatusError),

    #[error(transparent)]
    CodecError(#[from] CodecError),

    #[error(transparent)]
    ChunkError(#[from] re_chunk::ChunkError),

    #[error("Invalid URI: {0}")]
    InvalidUri(String),
}

// ----------------------------------------------------------------------------

const CATALOG_BP_STORE_ID: &str = "catalog_blueprint";
const CATALOG_REC_STORE_ID: &str = "catalog";
const CATALOG_APPLICATION_ID: &str = "redap_catalog";

/// Stream an rrd file or metadsasta catalog over gRPC from a Rerun Data Platform server.
///
/// `on_msg` can be used to wake up the UI thread on Wasm.
pub fn stream_from_redap(
    url: String,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<re_smart_channel::Receiver<LogMsg>, InvalidRedapAddress> {
    re_log::debug!("Loading {url}…");

    let address = url.as_str().try_into()?;

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RerunGrpcStream { url: url.clone() },
        re_smart_channel::SmartChannelSource::RerunGrpcStream { url: url.clone() },
    );

    spawn_future(async move {
        match address {
            RedapAddress::Recording {
                redap_endpoint,
                recording_id,
            } => {
                if let Err(err) =
                    stream_recording_async(tx, redap_endpoint, recording_id, on_msg).await
                {
                    re_log::warn!(
                        "Error while streaming {url}: {}",
                        re_error::format_ref(&err)
                    );
                }
            }
            RedapAddress::Catalog { redap_endpoint } => {
                if let Err(err) = stream_catalog_async(tx, redap_endpoint, on_msg).await {
                    re_log::warn!(
                        "Error while streaming {url}: {}",
                        re_error::format_ref(&err)
                    );
                }
            }
        }
    });

    Ok(rx)
}

#[cfg(target_arch = "wasm32")]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static + Send,
{
    tokio::spawn(future);
}

async fn stream_recording_async(
    tx: re_smart_channel::Sender<LogMsg>,
    redap_endpoint: Url,
    recording_id: String,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    use tokio_stream::StreamExt as _;

    re_log::debug!("Connecting to {redap_endpoint}…");
    let mut client = {
        #[cfg(target_arch = "wasm32")]
        let tonic_client = tonic_web_wasm_client::Client::new_with_options(
            redap_endpoint.to_string(),
            tonic_web_wasm_client::options::FetchOptions::new()
                .mode(tonic_web_wasm_client::options::Mode::Cors), // I'm not 100% sure this is needed, but it felt right.
        );

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = tonic::transport::Endpoint::new(redap_endpoint.to_string())?
            .connect()
            .await?;

        // TODO(#8411): figure out the right size for this
        StorageNodeClient::new(tonic_client).max_decoding_message_size(usize::MAX)
    };

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
                decode(r.encoder_version(), &r.payload)
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

    let store_info = store_info_from_catalog_chunk(&resp[0], &recording_id)?;
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
                decode(r.encoder_version(), &r.payload)
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
        let tc = result.map_err(TonicStatusError)?;
        let chunk = Chunk::from_transport(&tc)?;

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
    tc: &TransportChunk,
    recording_id: &str,
) -> Result<StoreInfo, StreamError> {
    let store_id = StoreId::from_string(StoreKind::Recording, recording_id.to_owned());

    let (_field, data) = tc
        .components()
        .find(|(f, _)| f.name == "application_id")
        .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: "no application_id field found".to_owned(),
        }))?;
    let app_id = data
        .as_any()
        .downcast_ref::<Arrow2Utf8Array<i32>>()
        .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: format!("application_id must be a utf8 array: {:?}", tc.schema),
        }))?
        .value(0);

    let (_field, data) = tc
        .components()
        .find(|(f, _)| f.name == "start_time")
        .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: "no start_time field found".to_owned(),
        }))?;
    let start_time = data
        .as_any()
        .downcast_ref::<arrow2::array::Int64Array>()
        .ok_or(StreamError::ChunkError(re_chunk::ChunkError::Malformed {
            reason: format!("start_time must be an int64 array: {:?}", tc.schema),
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

async fn stream_catalog_async(
    tx: re_smart_channel::Sender<LogMsg>,
    redap_endpoint: Url,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    use tokio_stream::StreamExt as _;

    re_log::debug!("Connecting to {redap_endpoint}…");
    let mut client = {
        #[cfg(target_arch = "wasm32")]
        let tonic_client = tonic_web_wasm_client::Client::new_with_options(
            redap_endpoint.to_string(),
            tonic_web_wasm_client::options::FetchOptions::new()
                .mode(tonic_web_wasm_client::options::Mode::Cors), // I'm not 100% sure this is needed, but it felt right.
        );

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = tonic::transport::Endpoint::new(redap_endpoint.to_string())?
            .connect()
            .await?;

        StorageNodeClient::new(tonic_client)
    };

    re_log::debug!("Fetching catalog…");

    let mut resp = client
        .query_catalog(QueryCatalogRequest {
            column_projection: None, // fetch all columns
            filter: None,            // fetch all rows
        })
        .await
        .map_err(TonicStatusError)?
        .into_inner()
        .map(|resp| {
            resp.and_then(|r| {
                decode(r.encoder_version(), &r.payload)
                    .map_err(|err| tonic::Status::internal(err.to_string()))
            })
        });

    drop(client);

    if activate_catalog_blueprint(&tx).is_err() {
        re_log::debug!("Failed to activate catalog blueprint");
        return Ok(());
    }

    // Craft the StoreInfo for the actual catalog data
    let store_id = StoreId::from_string(StoreKind::Recording, CATALOG_REC_STORE_ID.to_owned());

    let store_info = StoreInfo {
        application_id: ApplicationId::from(CATALOG_APPLICATION_ID),
        store_id: store_id.clone(),
        cloned_from: None,
        is_official_example: false,
        started: Time::now(),
        store_source: StoreSource::Unknown,
        store_version: None,
    };

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
        let input = result.map_err(TonicStatusError)?;

        // Catalog received from the ReDap server isn't suitable for direct conversion to a Rerun Chunk:
        // - conversion expects "data" columns to be ListArrays, hence we need to convert any individual row column data to ListArray
        // - conversion expects the input TransportChunk to have a ChunkId so we need to add that piece of metadata

        let mut fields = Vec::new();
        let mut arrays = Vec::new();
        // add the (row id) control field
        let (row_id_field, row_id_data) = input.controls().next().ok_or(
            StreamError::ChunkError(re_chunk::ChunkError::Malformed {
                reason: "no control field found".to_owned(),
            }),
        )?;

        fields.push(
            Arrow2Field::new(
                RowId::name().to_string(), // need to rename to Rerun Chunk expected control field
                row_id_field.data_type().clone(),
                false, /* not nullable */
            )
            .with_metadata(TransportChunk::field_metadata_control_column()),
        );
        arrays.push(row_id_data.clone());

        // next add any timeline field
        for (field, data) in input.timelines() {
            fields.push(field.clone());
            arrays.push(data.clone());
        }

        // now add all the 'data' fields - we slice each column array into individual arrays and then convert the whole lot into a ListArray
        for (field, data) in input.components() {
            let data_field_inner =
                Arrow2Field::new("item", field.data_type().clone(), true /* nullable */);

            let data_field = Arrow2Field::new(
                field.name.clone(),
                arrow2::datatypes::DataType::List(Arc::new(data_field_inner.clone())),
                false, /* not nullable */
            )
            .with_metadata(TransportChunk::field_metadata_data_column());

            let mut sliced: Vec<Box<dyn Arrow2Array>> = Vec::new();
            for idx in 0..data.len() {
                let mut array = data.clone();
                array.slice(idx, 1);
                sliced.push(array);
            }

            let data_arrays = sliced.iter().map(|e| Some(e.as_ref())).collect::<Vec<_>>();
            #[allow(clippy::unwrap_used)] // we know we've given the right field type
            let data_field_array: arrow2::array::ListArray<i32> =
                re_chunk::util::arrays_to_list_array(
                    data_field_inner.data_type().clone(),
                    &data_arrays,
                )
                .unwrap();

            fields.push(data_field);
            arrays.push(Box::new(data_field_array));
        }

        let mut schema = arrow2::datatypes::Schema::from(fields);
        schema.metadata.insert(
            TransportChunk::CHUNK_METADATA_KEY_ENTITY_PATH.to_owned(),
            "catalog".to_owned(),
        );

        // modified and enriched TransportChunk
        let mut tc = TransportChunk {
            schema,
            data: arrow2::chunk::Chunk::new(arrays),
        };

        tc.schema.metadata.insert(
            TransportChunk::CHUNK_METADATA_KEY_ID.to_owned(),
            ChunkId::new().to_string(),
        );
        let mut chunk = Chunk::from_transport(&tc)?;

        // finally, enrich catalog data with RecordingUri that's based on the ReDap endpoint (that we know)
        // and the recording id (that we have in the catalog data)
        let host = redap_endpoint
            .host()
            .ok_or(StreamError::InvalidUri(format!(
                "couldn't get host from {redap_endpoint}"
            )))?;
        let port = redap_endpoint
            .port()
            .ok_or(StreamError::InvalidUri(format!(
                "couldn't get port from {redap_endpoint}"
            )))?;

        let recording_uri_arrays: Vec<Box<dyn Arrow2Array>> = chunk
            .iter_string(&"id".into())
            .map(|id| {
                let rec_id = &id[0]; // each component batch is of length 1 i.e. single 'id' value

                let recording_uri = format!("rerun://{host}:{port}/recording/{rec_id}");

                let recording_uri_data = Arrow2Utf8Array::<i32>::from([Some(recording_uri)]);

                Ok::<Box<_>, StreamError>(
                    Box::new(recording_uri_data) as Box<dyn arrow2::array::Array>
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let recording_id_arrays = recording_uri_arrays
            .iter()
            .map(|e| Some(e.as_ref()))
            .collect::<Vec<_>>();

        let rec_id_field = Arrow2Field::new("item", arrow2::datatypes::DataType::Utf8, true);
        #[allow(clippy::unwrap_used)] // we know we've given the right field type
        let uris = re_chunk::util::arrays_to_list_array(
            rec_id_field.data_type().clone(),
            &recording_id_arrays,
        )
        .unwrap();

        chunk.add_component(ComponentDescriptor::new(RecordingUri::name()), uris)?;

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

// Craft a blueprint from relevant chunks and activate it
// TODO(zehiko) - manual crafting of the blueprint as we have below will go away and be replaced
// by either a blueprint crafted using rust Blueprint API or a blueprint fetched from ReDap (#8470)
fn activate_catalog_blueprint(
    tx: &re_smart_channel::Sender<LogMsg>,
) -> Result<(), Box<dyn std::error::Error>> {
    let blueprint_store_id =
        StoreId::from_string(StoreKind::Blueprint, CATALOG_BP_STORE_ID.to_owned());
    let blueprint_store_info = StoreInfo {
        application_id: ApplicationId::from(CATALOG_APPLICATION_ID),
        store_id: blueprint_store_id.clone(),
        cloned_from: None,
        is_official_example: false,
        started: Time::now(),
        store_source: StoreSource::Unknown,
        store_version: None,
    };

    if tx
        .send(LogMsg::SetStoreInfo(SetStoreInfo {
            row_id: *re_chunk::RowId::new(),
            info: blueprint_store_info,
        }))
        .is_err()
    {
        re_log::debug!("Receiver disconnected");
        return Ok(());
    }

    let timepoint = [(Timeline::new_sequence("blueprint"), 1)];

    let vb = ViewBlueprint::new("Dataframe")
        .with_visible(true)
        .with_space_origin("/");

    // TODO(zehiko) we shouldn't really be creating all these ids and entity paths manually... (#8470)
    let view_uuid = uuid::Uuid::new_v4();
    let view_entity_path = format!("/view/{view_uuid}");
    let view_chunk = ChunkBuilder::new(ChunkId::new(), view_entity_path.clone().into())
        .with_archetype(RowId::new(), timepoint, &vb)
        .build()?;

    let epf = EntityPathFilter::parse_forgiving("/**", &Default::default());
    let vc = ViewContents::new(epf.iter_expressions());
    let view_contents_chunk = ChunkBuilder::new(
        ChunkId::new(),
        format!(
            "{}/{}",
            view_entity_path.clone(),
            ViewContents::name().short_name()
        )
        .into(),
    )
    .with_archetype(RowId::new(), timepoint, &vc)
    .build()?;

    let rc = ContainerBlueprint::new(ContainerKind::Grid)
        .with_contents(&[EntityPath::from(view_entity_path)])
        .with_visible(true);

    let container_uuid = uuid::Uuid::new_v4();
    let container_chunk = ChunkBuilder::new(
        ChunkId::new(),
        format!("/container/{container_uuid}").into(),
    )
    .with_archetype(RowId::new(), timepoint, &rc)
    .build()?;

    let vp = ViewportBlueprint::new().with_root_container(RootContainer(container_uuid.into()));
    let viewport_chunk = ChunkBuilder::new(ChunkId::new(), "/viewport".into())
        .with_archetype(RowId::new(), timepoint, &vp)
        .build()?;

    for chunk in &[
        view_chunk,
        view_contents_chunk,
        container_chunk,
        viewport_chunk,
    ] {
        if tx
            .send(LogMsg::ArrowMsg(
                blueprint_store_id.clone(),
                chunk.to_arrow_msg()?,
            ))
            .is_err()
        {
            re_log::debug!("Receiver disconnected");
            return Ok(());
        }
    }

    let blueprint_activation = BlueprintActivationCommand {
        blueprint_id: blueprint_store_id.clone(),
        make_active: true,
        make_default: true,
    };

    if tx
        .send(LogMsg::BlueprintActivationCommand(blueprint_activation))
        .is_err()
    {
        re_log::debug!("Receiver disconnected");
        return Ok(());
    }

    Ok(())
}
