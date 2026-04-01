use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

/// Container for a Python exception that can be sent across threads
/// and later re-raised at the PyO3 boundary.
///
/// Cloning requires the GIL (via [`PyErr::clone_ref`]).
pub struct PythonException {
    reason: String,
    inner: pyo3::PyErr,
}

impl PythonException {
    pub fn new(err: pyo3::PyErr) -> Self {
        let reason = err.to_string();
        Self { reason, inner: err }
    }

    pub fn into_py_err(self) -> pyo3::PyErr {
        self.inner
    }
}

impl Clone for PythonException {
    fn clone(&self) -> Self {
        let inner = Python::attach(|py| self.inner.clone_ref(py));
        Self {
            reason: self.reason.clone(),
            inner,
        }
    }
}

impl std::fmt::Debug for PythonException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PythonException")
            .field("reason", &self.reason)
            .finish()
    }
}

impl std::fmt::Display for PythonException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum ChunkPipelineError {
    #[error("Failed to decode chunk from RRD file: {reason}")]
    RrdChunkDecode { reason: String },

    #[error("Failed to read RRD file: {reason}")]
    RrdRead { reason: String },

    #[error("{0}")]
    PythonIterator(PythonException),
}

impl From<ChunkPipelineError> for pyo3::PyErr {
    fn from(err: ChunkPipelineError) -> Self {
        match err {
            ChunkPipelineError::PythonIterator(exc) => exc.into_py_err(),

            other => PyRuntimeError::new_err(other.to_string()),
        }
    }
}
