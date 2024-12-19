#![allow(unsafe_op_in_unsafe_fn)]
use arrow::{
    array::{RecordBatch, RecordBatchIterator, RecordBatchReader},
    datatypes::Schema,
    ffi_stream::ArrowArrayStreamReader,
    pyarrow::PyArrowType,
};
// False positive due to #[pyfunction] macro
use pyo3::{exceptions::PyRuntimeError, prelude::*, Bound, PyResult};
use re_chunk::{Chunk, TransportChunk};
use re_chunk_store::ChunkStore;
use re_dataframe::ChunkStoreHandle;
use re_log_encoding::codec::wire::{decode, encode};
use re_log_types::{StoreInfo, StoreSource};
use re_protos::{
    common::v0::{EncoderVersion, RecordingId},
    remote_store::v0::{
        storage_node_client::StorageNodeClient, DataframePart, FetchRecordingRequest,
        QueryCatalogRequest, RecordingType, RegisterRecordingRequest, UpdateCatalogRequest,
    },
};
use re_sdk::{ApplicationId, StoreId, StoreKind, Time};
use tokio_stream::StreamExt;

use crate::dataframe::PyRecording;

/// Register the `rerun.remote` module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStorageNodeClient>()?;

    m.add_function(wrap_pyfunction!(connect, m)?)?;

    Ok(())
}

async fn connect_async(addr: String) -> PyResult<StorageNodeClient<tonic::transport::Channel>> {
    #[cfg(not(target_arch = "wasm32"))]
    let tonic_client = tonic::transport::Endpoint::new(addr)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
        .connect()
        .await
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    Ok(StorageNodeClient::new(tonic_client))
}

/// Load a rerun archive from an RRD file.
///
/// Required-feature: `remote`
///
/// Parameters
/// ----------
/// addr : str
///     The address of the storage node to connect to.
///
/// Returns
/// -------
/// StorageNodeClient
///     The connected client.
#[pyfunction]
pub fn connect(addr: String) -> PyResult<PyStorageNodeClient> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let client = runtime.block_on(connect_async(addr))?;

    Ok(PyStorageNodeClient { runtime, client })
}

/// A connection to a remote storage node.
#[pyclass(name = "StorageNodeClient")]
pub struct PyStorageNodeClient {
    /// A tokio runtime for async operations. This connection will currently
    /// block the Python interpreter while waiting for responses.
    /// This runtime must be persisted for the lifetime of the connection.
    runtime: tokio::runtime::Runtime,

    /// The actual tonic connection.
    client: StorageNodeClient<tonic::transport::Channel>,
}

#[pymethods]
impl PyStorageNodeClient {
    /// Get the metadata for all recordings in the storage node.
    fn query_catalog(&mut self) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        let reader = self.runtime.block_on(async {
            // TODO(jleibs): Support column projection and filtering
            let request = QueryCatalogRequest {
                column_projection: None,
                filter: None,
            };

            let transport_chunks = self
                .client
                .query_catalog(request)
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner()
                .filter_map(|resp| {
                    resp.and_then(|r| {
                        decode(r.encoder_version(), &r.payload)
                            .map_err(|err| tonic::Status::internal(err.to_string()))
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let record_batches: Vec<Result<RecordBatch, arrow::error::ArrowError>> =
                transport_chunks
                    .into_iter()
                    .map(|tc| tc.try_to_arrow_record_batch())
                    .collect();

            // TODO(jleibs): surfacing this schema is awkward. This should be more explicit in
            // the gRPC APIs somehow.
            let schema = record_batches
                .first()
                .and_then(|batch| batch.as_ref().ok().map(|batch| batch.schema()))
                .unwrap_or(std::sync::Arc::new(Schema::empty()));

            let reader = RecordBatchIterator::new(record_batches, schema);

            Ok::<_, PyErr>(reader)
        })?;

        Ok(PyArrowType(Box::new(reader)))
    }

    /// Register a recording along with some metadata.
    ///
    /// Parameters
    /// ----------
    /// storage_url : str
    ///     The URL to the storage location.
    /// metadata : Optional[Table | RecordBatch]
    ///     A pyarrow Table or RecordBatch containing the metadata to update.
    ///     This Table must contain only a single row.
    #[pyo3(signature = (
        storage_url,
        metadata = None
    ))]
    fn register(&mut self, storage_url: &str, metadata: Option<MetadataLike>) -> PyResult<String> {
        self.runtime.block_on(async {
            let storage_url = url::Url::parse(storage_url)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let _obj = object_store::ObjectStoreScheme::parse(&storage_url)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let metadata = metadata
                .map(|metadata| {
                    let metadata = metadata.into_record_batch()?;

                    if metadata.num_rows() != 1 {
                        return Err(PyRuntimeError::new_err(
                            "Metadata must contain exactly one row",
                        ));
                    }

                    let metadata_tc = TransportChunk::from_arrow_record_batch(&metadata);

                    encode(EncoderVersion::V0, metadata_tc)
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
                })
                .transpose()?
                .map(|payload| DataframePart {
                    encoder_version: EncoderVersion::V0 as i32,
                    payload,
                });

            let request = RegisterRecordingRequest {
                // TODO(jleibs): Description should really just be in the metadata
                description: Default::default(),
                storage_url: storage_url.to_string(),
                metadata,
                typ: RecordingType::Rrd.into(),
            };

            let resp = self
                .client
                .register_recording(request)
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner();
            let metadata = decode(resp.encoder_version(), &resp.payload)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?

            let recording_id = metadata
                .all_columns()
                .find(|(field, _data)| field.name == "id")
                .map(|(_field, data)| data)
                .ok_or(PyRuntimeError::new_err("No id"))?
                .as_any()
                .downcast_ref::<arrow2::array::Utf8Array<i32>>()
                .ok_or(PyRuntimeError::new_err("Id is not a string"))?
                .value(0)
                .to_owned();

            Ok(recording_id)
        })
    }

    /// Update the catalog metadata for one or more recordings.
    ///
    /// The updates are provided as a pyarrow Table or RecordBatch containing the metadata to update.
    /// The Table must contain an 'id' column, which is used to specify the recording to update for each row.
    ///
    /// Parameters
    /// ----------
    /// metadata : Table | RecordBatch
    ///     A pyarrow Table or RecordBatch containing the metadata to update.
    #[pyo3(signature = (
        metadata
    ))]
    #[allow(clippy::needless_pass_by_value)]
    fn update_catalog(&mut self, metadata: MetadataLike) -> PyResult<()> {
        self.runtime.block_on(async {
            let metadata = metadata.into_record_batch()?;

            // TODO(jleibs): This id name should probably come from `re_protos`
            if metadata.schema().column_with_name("id").is_none() {
                return Err(PyRuntimeError::new_err(
                    "Metadata must contain an 'id' column",
                ));
            }

            let metadata_tc = TransportChunk::from_arrow_record_batch(&metadata);

            let request = UpdateCatalogRequest {
                metadata: Some(DataframePart {
                    encoder_version: EncoderVersion::V0 as i32,
                    payload: encode(EncoderVersion::V0, metadata_tc)
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
                }),
            };

            self.client
                .update_catalog(request)
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            Ok(())
        })
    }

    /// Open a [`Recording`][rerun.dataframe.Recording] by id to use with the dataframe APIs.
    ///
    /// This currently downloads the full recording to the local machine.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     The id of the recording to open.
    ///
    /// Returns
    /// -------
    /// Recording
    ///     The opened recording.
    #[pyo3(signature = (
        id,
    ))]
    fn open_recording(&mut self, id: &str) -> PyResult<PyRecording> {
        use tokio_stream::StreamExt as _;
        let store = self.runtime.block_on(async {
            let mut resp = self
                .client
                .fetch_recording(FetchRecordingRequest {
                    recording_id: Some(RecordingId { id: id.to_owned() }),
                })
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner();

            // TODO(jleibs): Does this come from RDP?
            let store_id = StoreId::from_string(StoreKind::Recording, id.to_owned());

            let store_info = StoreInfo {
                application_id: ApplicationId::from("rerun_data_platform"),
                store_id: store_id.clone(),
                cloned_from: None,
                is_official_example: false,
                started: Time::now(),
                store_source: StoreSource::Unknown,
                store_version: None,
            };

            let mut store = ChunkStore::new(store_id, Default::default());
            store.set_info(store_info);

            while let Some(result) = resp.next().await {
                let response = result.map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
                let tc = decode(EncoderVersion::V0, &response.payload)
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

                let Some(tc) = tc else {
                    return Err(PyRuntimeError::new_err("Stream error"));
                };

                let chunk = Chunk::from_transport(&tc)
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

                store
                    .insert_chunk(&std::sync::Arc::new(chunk))
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
            }

            Ok(store)
        })?;

        let handle = ChunkStoreHandle::new(store);

        let cache =
            re_dataframe::QueryCacheHandle::new(re_dataframe::QueryCache::new(handle.clone()));

        Ok(PyRecording {
            store: handle,
            cache,
        })
    }
}

/// A type alias for metadata.
#[derive(FromPyObject)]
enum MetadataLike {
    RecordBatch(PyArrowType<RecordBatch>),
    Reader(PyArrowType<ArrowArrayStreamReader>),
}

impl MetadataLike {
    fn into_record_batch(self) -> PyResult<RecordBatch> {
        let (schema, batches) = match self {
            Self::RecordBatch(record_batch) => (record_batch.0.schema(), vec![record_batch.0]),
            Self::Reader(reader) => (
                reader.0.schema(),
                reader.0.collect::<Result<Vec<_>, _>>().map_err(|err| {
                    PyRuntimeError::new_err(format!("Failed to read RecordBatches: {err}"))
                })?,
            ),
        };

        arrow::compute::concat_batches(&schema, &batches)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    }
}
