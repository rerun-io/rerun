//! A background task uploads the RRD and submits registration, then publishes its
//! handle or failure through a watch channel. Dropping a waiter does not stop this task.
//! Waiters query registration on demand and cache only terminal outcomes in a OnceLock;
//! timeouts and query failures leave registration retryable.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use pyo3::exceptions::{PyKeyboardInterrupt, PyOSError, PyRuntimeError, PyValueError};
use pyo3::{PyErr, PyResult, Python, pyclass, pymethods};
use re_catalog_client::{RegistrationError, StagingError, WriteError, WriteHandle};
use re_redap_client::ApiError;
use tokio::sync::watch;
use tracing::Instrument as _;

use super::registration_handle::DEFAULT_TIMEOUT_SECS;
use super::to_py_err;
use crate::trace_context::read_trace_context_from_python;
use crate::utils::{get_tokio_runtime, wait_for_future};

// TODO(RR-6038): Expose upload progress and richer registration status.
#[derive(Clone)]
enum WriteState {
    Uploading,
    Registering(WriteHandle),
    Failed(Arc<WriteError>),
}

/// Segment IDs and storage URL of a registered write.
type WriteResult = (Vec<String>, String);

#[pyclass(
    name = "WriteOperationInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyWriteOperationInternal {
    state: watch::Receiver<WriteState>,

    /// Set once registration succeeds or a task fails; an unknown outcome leaves it unset.
    registration: OnceLock<Result<WriteResult, ApiError>>,
}

impl PyWriteOperationInternal {
    pub fn new(
        upload: impl Future<Output = Result<WriteHandle, Box<WriteError>>> + Send + 'static,
    ) -> Self {
        let (sender, state) = watch::channel(WriteState::Uploading);
        get_tokio_runtime().spawn(
            async move {
                sender.send_replace(match upload.await {
                    Ok(registration) => WriteState::Registering(registration),
                    Err(err) => WriteState::Failed(Arc::from(err)),
                });
            }
            .in_current_span(),
        );
        Self {
            state,
            registration: OnceLock::new(),
        }
    }

    async fn wait_for_outcome(&self, timeout: Duration) -> PyResult<WriteResult> {
        let mut state = self.state.clone();
        loop {
            if let Some(registration) = self.registration.get() {
                return registration.clone().map_err(to_py_err);
            }
            let current = state.borrow_and_update().clone();
            match current {
                WriteState::Uploading => {}
                WriteState::Registering(write) => {
                    let registration = match write.wait(timeout).await {
                        Ok(segment_ids) => Ok((
                            segment_ids.into_iter().map(|id| id.to_string()).collect(),
                            write.storage_url.to_string(),
                        )),
                        Err(RegistrationError::Query(err)) => return Err(to_py_err(err)),
                        Err(err @ RegistrationError::Failed(_)) => Err(err.into()),
                    };
                    return self
                        .registration
                        .get_or_init(|| registration)
                        .clone()
                        .map_err(to_py_err);
                }
                WriteState::Failed(err) => return Err(write_error_to_py(&err)),
            }
            state.changed().await.map_err(|err| {
                PyRuntimeError::new_err(format!(
                    "write worker stopped before reporting an outcome; remote outcome is unknown: {err}"
                ))
            })?;
        }
    }
}

#[pymethods]
impl PyWriteOperationInternal {
    fn status(&self) -> &'static str {
        match self.registration.get() {
            Some(Ok(_)) => return "succeeded",
            Some(Err(_)) => return "failed",
            None => {}
        }
        match &*self.state.borrow() {
            WriteState::Registering(_) => "registering",
            WriteState::Failed(_) => "failed",
            WriteState::Uploading if self.state.has_changed().is_err() => "failed",
            WriteState::Uploading => "uploading",
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
                    result = self.wait_for_outcome(timeout) => result,
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
        WriteError::Upload(StagingError::Api(source)) => to_py_err(source.clone()),
        WriteError::Upload(_) => PyRuntimeError::new_err(err.to_string()),
    }
}
