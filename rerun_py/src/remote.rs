#![allow(unsafe_op_in_unsafe_fn)]
use arrow::{array::ArrayData, pyarrow::PyArrowType};
// False positive due to #[pyfunction] macro
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyDict, Bound, PyResult};
use re_chunk::TransportChunk;
use re_protos::v0::{
    storage_node_client::StorageNodeClient, EncoderVersion, ListRecordingsRequest,
    RecordingMetadata, RecordingType, RegisterRecordingRequest,
};

/// Register the `rerun.remote` module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
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

#[pyfunction]
pub fn connect(addr: String) -> PyResult<PyConnection> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let client = runtime.block_on(connect_async(addr))?;

    Ok(PyConnection { runtime, client })
}

/// A connection to a remote storage node.
#[pyclass(name = "Connection")]
pub struct PyConnection {
    /// A tokio runtime for async operations. This connection will currently
    /// block the Python interpreter while waiting for responses.
    /// This runtime must be persisted for the lifetime of the connection.
    runtime: tokio::runtime::Runtime,

    /// The actual tonic connection.
    client: StorageNodeClient<tonic::transport::Channel>,
}

#[pymethods]
impl PyConnection {
    /// List all recordings registered with the node.
    fn list_recordings(&mut self) -> PyResult<Vec<PyRecordingMetadata>> {
        self.runtime.block_on(async {
            let request = ListRecordingsRequest {
                column_projection: None,
            };

            let resp = self
                .client
                .list_recordings(request)
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            Ok(resp
                .into_inner()
                .recordings
                .into_iter()
                .map(|recording| PyRecordingMetadata { info: recording })
                .collect())
        })
    }

    /// Register a recording along with some metadata
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

/// The info for a recording stored in the archive.
#[pyclass(name = "RecordingMetadata")]
pub struct PyRecordingMetadata {
    info: re_protos::v0::RecordingMetadata,
}

#[pymethods]
impl PyRecordingMetadata {
    fn __repr__(&self) -> String {
        format!(
            "Recording(id={})",
            self.info
                .id()
                .map(|id| id.to_string())
                .unwrap_or("Unknown".to_owned())
        )
    }
}
