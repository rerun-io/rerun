#![allow(unsafe_op_in_unsafe_fn)]
use arrow::{
    array::{ArrayData, RecordBatch, RecordBatchIterator, RecordBatchReader},
    datatypes::Schema,
    pyarrow::PyArrowType,
};
// False positive due to #[pyfunction] macro
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyDict, Bound, PyResult};
use re_chunk::{Chunk, TransportChunk};
use re_chunk_store::ChunkStore;
use re_dataframe::ChunkStoreHandle;
use re_log_types::{StoreInfo, StoreSource};
use re_protos::{
    codec::decode,
    v0::{
        storage_node_client::StorageNodeClient, EncoderVersion, FetchRecordingRequest,
        QueryCatalogRequest, RecordingId, RecordingMetadata, RecordingType,
        RegisterRecordingRequest, UpdateCatalogRequest,
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
    /// Query the recordings metadata catalog.
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
                    .transpose()
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
    /// metadata : dict[str, MetadataLike]
    ///     A dictionary where the keys are the metadata columns and the values are pyarrow arrays.
    #[pyo3(signature = (
        storage_url,
        metadata = None
    ))]
    fn register(
        &mut self,
        storage_url: &str,
        metadata: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<String> {
        self.runtime.block_on(async {
            let storage_url = url::Url::parse(storage_url)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let _obj = object_store::ObjectStoreScheme::parse(&storage_url)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let metadata = metadata
                .map(|metadata| {
                    let (schema, data): (
                        Vec<arrow2::datatypes::Field>,
                        Vec<Box<dyn arrow2::array::Array>>,
                    ) = metadata
                        .iter()
                        .map(|(key, value)| {
                            let key = key.to_string();
                            let value = value.extract::<MetadataLike>()?;
                            let value_array = value.to_arrow2()?;
                            let field = arrow2::datatypes::Field::new(
                                key,
                                value_array.data_type().clone(),
                                true,
                            );
                            Ok((field, value_array))
                        })
                        .collect::<PyResult<Vec<_>>>()?
                        .into_iter()
                        .unzip();

                    let schema = arrow2::datatypes::Schema::from(schema);

                    let data = arrow2::chunk::Chunk::new(data);

                    let metadata_tc = TransportChunk {
                        schema: schema.clone(),
                        data,
                    };

                    RecordingMetadata::try_from(EncoderVersion::V0, &metadata_tc)
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
                })
                .transpose()?;

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

            let recording_id: String = resp.id.map_or("Unknown".to_owned(), |id| id.id);

            Ok(recording_id)
        })
    }

    /// Update the metadata for the recording with the given id.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     The id of the recording to update.
    /// metadata : dict[str, MetadataLike]
    ///     A dictionary where the keys are the metadata columns and the values are pyarrow arrays.
    #[pyo3(signature = (
        id,
        metadata
    ))]
    fn update_catalog(&mut self, id: &str, metadata: &Bound<'_, PyDict>) -> PyResult<()> {
        self.runtime.block_on(async {
            let (schema, data): (
                Vec<arrow2::datatypes::Field>,
                Vec<Box<dyn arrow2::array::Array>>,
            ) = metadata
                .iter()
                .map(|(key, value)| {
                    let key = key.to_string();
                    let value = value.extract::<MetadataLike>()?;
                    let value_array = value.to_arrow2()?;
                    let field =
                        arrow2::datatypes::Field::new(key, value_array.data_type().clone(), true);
                    Ok((field, value_array))
                })
                .collect::<PyResult<Vec<_>>>()?
                .into_iter()
                .unzip();

            let schema = arrow2::datatypes::Schema::from(schema);

            let data = arrow2::chunk::Chunk::new(data);

            let metadata_tc = TransportChunk {
                schema: schema.clone(),
                data,
            };

            let metadata = RecordingMetadata::try_from(EncoderVersion::V0, &metadata_tc)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let request = UpdateCatalogRequest {
                recording_id: Some(RecordingId { id: id.to_owned() }),
                metadata: Some(metadata),
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
    PyArrow(PyArrowType<ArrayData>),
    // TODO(jleibs): Support converting other primitives
}

impl MetadataLike {
    fn to_arrow2(&self) -> PyResult<Box<dyn re_chunk::Arrow2Array>> {
        match self {
            Self::PyArrow(array) => {
                let array = arrow2::array::from_data(&array.0);
                if array.len() == 1 {
                    Ok(array)
                } else {
                    Err(PyRuntimeError::new_err(
                        "Metadata must be a single array, not a list",
                    ))
                }
            }
        }
    }

    #[allow(dead_code)]
    fn to_arrow(&self) -> PyResult<std::sync::Arc<dyn arrow::array::Array>> {
        match self {
            Self::PyArrow(array) => {
                let array = arrow::array::make_array(array.0.clone());
                if array.len() == 1 {
                    Ok(array)
                } else {
                    Err(PyRuntimeError::new_err(
                        "Metadata must be a single array, not a list",
                    ))
                }
            }
        }
    }
}
