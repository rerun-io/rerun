#![allow(unsafe_op_in_unsafe_fn)]
// False positive due to #[pyfunction] macro
use pyo3::{exceptions::PyRuntimeError, prelude::*, Bound, PyResult};
use re_remote_store_types::v0::{storage_node_client::StorageNodeClient, ListRecordingsRequest};

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
    fn list_recordings(&mut self) -> PyResult<Vec<PyRecordingInfo>> {
        self.runtime.block_on(async {
            let request = ListRecordingsRequest {};

            let resp = self
                .client
                .list_recordings(request)
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            Ok(resp
                .into_inner()
                .recordings
                .into_iter()
                .map(|recording| PyRecordingInfo { info: recording })
                .collect())
        })
    }
}

/// The info for a recording stored in the archive.
#[pyclass(name = "RecordingInfo")]
pub struct PyRecordingInfo {
    info: re_remote_store_types::v0::RecordingInfo,
}

#[pymethods]
impl PyRecordingInfo {
    fn __repr__(&self) -> String {
        format!(
            "Recording(id={})",
            self.info.id.as_ref().map_or("Unknown", |id| id.id.as_str())
        )
    }
}
