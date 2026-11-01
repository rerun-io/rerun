use std::time::Duration;

use pyo3::exceptions::{PyKeyboardInterrupt, PyOSError, PyRuntimeError, PyValueError};
use pyo3::{PyErr, PyResult, Python, pyclass, pymethods};
use re_catalog_client::{
    StagingError, WriteError, WriteOperation, WriteOperationError, WriteStatus,
};
use re_redap_client::ApiError;
use tracing::Instrument as _;

use super::registration_handle::DEFAULT_TIMEOUT_SECS;
use super::to_py_err;
use crate::trace_context::read_trace_context_from_python;
use crate::utils::wait_for_future;

/// Segment IDs and storage URL of a registered write.
type WriteResult = (Vec<String>, String);

#[pyclass(
    name = "WriteOperationInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyWriteOperationInternal {
    operation: WriteOperation,
}

impl PyWriteOperationInternal {
    pub fn new(operation: WriteOperation) -> Self {
        Self { operation }
    }
}

#[pymethods]
impl PyWriteOperationInternal {
    fn status(&self) -> &'static str {
        match self.operation.status() {
            WriteStatus::Uploading => "uploading",
            WriteStatus::Registering => "registering",
            WriteStatus::Succeeded => "succeeded",
            WriteStatus::Failed => "failed",
        }
    }

    #[pyo3(signature = (timeout_secs=None))]
    fn wait(&self, py: Python<'_>, timeout_secs: Option<u64>) -> PyResult<WriteResult> {
        let span = read_trace_context_from_python(py, "WriteOperation.wait");
        let timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS));
        wait_for_future(
            py,
            async {
                // The upload has no deadline, so Ctrl-C is the only way to stop waiting for it.
                tokio::select! {
                    result = self.operation.wait(timeout) => result
                        .map(|result| (
                            result.segment_ids.into_iter().map(|id| id.to_string()).collect(),
                            result.storage_url.to_string(),
                        ))
                        .map_err(|err| match err {
                            WriteOperationError::Write(err) => write_error_to_py(&err),
                            WriteOperationError::RegistrationFailed(err)
                            | WriteOperationError::RegistrationQuery(err) => to_py_err(err),
                            WriteOperationError::WorkerStopped => PyRuntimeError::new_err(err.to_string()),
                        }),
                    Ok(()) = tokio::signal::ctrl_c() => Err(PyKeyboardInterrupt::new_err(None::<()>)),
                }
            }
            .instrument(span),
        )
    }
}

fn write_error_to_py(err: &WriteError) -> PyErr {
    match err {
        WriteError::Key(err) => PyValueError::new_err(err.to_string()),
        WriteError::Open { .. } | WriteError::Upload(StagingError::Read(_)) => {
            PyOSError::new_err(err.to_string())
        }
        WriteError::Register {
            dataset_id,
            storage_url,
            source,
        } => {
            let mut contextual = ApiError::clone(source);
            contextual.message = re_error::format_with_details(
                format!("failed to register uploaded RRD: {}", source.message),
                format!("Dataset: {dataset_id}\nStaged URL: {storage_url}"),
            );
            to_py_err(contextual)
        }
        WriteError::ResolveDataset { source, .. } => {
            let mut contextual = ApiError::clone(source);
            contextual.message = err.to_string();
            to_py_err(contextual)
        }
        WriteError::Upload(StagingError::Api(source)) => to_py_err(source.clone()),
        WriteError::Upload(_) => PyRuntimeError::new_err(err.to_string()),
    }
}
