use std::sync::Arc;

use datafusion::datasource::MemTable;
use pyo3::exceptions::PyIndexError;
use pyo3::{pyclass, pymethods, Py, PyRef, PyResult, Python};

use re_log_types::hash::Hash64;
use re_protos::common::v1alpha1::TaskId;

use crate::catalog::{to_py_err, PyCatalogClient};
use crate::dataframe::PyDataFusionTable;

/// A handle on a remote task.
#[pyclass(name = "Task")]
pub struct PyTask {
    pub client: Py<PyCatalogClient>,

    pub id: TaskId,
}

/// A handle on a remote task.
#[pymethods]
impl PyTask {
    /// Entry id as a string.
    pub fn __repr__(&self) -> String {
        format!("Task({})", self.id.id)
    }

    /// The task id.
    #[getter]
    pub fn id(&self) -> String {
        self.id.id.clone()
    }

    /// Block until the task is completed or the timeout is reached.
    ///
    /// A `TimeoutError` is raised if the timeout is reached.
    pub fn wait(&self, py: Python<'_>, timeout_secs: u64) -> PyResult<()> {
        let mut connection = self.client.borrow(py).connection().clone();
        let timeout = std::time::Duration::from_secs(timeout_secs);
        connection.wait_for_tasks(py, &[self.id.clone()], timeout)?;

        Ok(())
    }

    //TODO(ab): add method to poll about status
}

/// A collection of [`Task`].
#[allow(rustdoc::broken_intra_doc_links)]
#[pyclass(name = "Tasks")]
pub struct PyTasks {
    pub client: Py<PyCatalogClient>,

    pub ids: Vec<TaskId>,
}

#[pymethods]
impl PyTasks {
    /// Block until all tasks are completed or the timeout is reached.
    ///
    /// A `TimeoutError` is raised if the timeout is reached.
    pub fn wait(self_: PyRef<'_, Self>, timeout_secs: u64) -> PyResult<()> {
        let mut connection = self_.client.borrow(self_.py()).connection().clone();
        let timeout = std::time::Duration::from_secs(timeout_secs);
        connection.wait_for_tasks(self_.py(), &self_.ids, timeout)?;

        Ok(())
    }

    /// Return a table with the status of all tasks.
    pub fn status_table(&self, py: Python<'_>) -> PyResult<PyDataFusionTable> {
        let mut connection = self.client.borrow(py).connection().clone();

        // TODO(dataplatform/issues#709): we'd use `OperationId` here if we had it.
        let hash = Hash64::hash(self.ids.iter().map(|id| id.id.as_str()).collect::<Vec<_>>());
        let name = format!("__tasks_{:x}__", hash.hash64());

        let task_status_table = connection.query_tasks(py, &self.ids)?;
        let provider = MemTable::try_new(task_status_table.schema(), vec![vec![task_status_table]])
            .map_err(to_py_err)?;

        Ok(PyDataFusionTable {
            provider: Arc::new(provider),
            name,
            client: self.client.clone_ref(py),
        })
    }

    //
    // Sequence methods
    //

    fn __len__(&self) -> usize {
        self.ids.len()
    }

    /// Get the task at the given index.
    fn __getitem__(&self, py: Python<'_>, index: usize) -> PyResult<PyTask> {
        if index >= self.ids.len() {
            return Err(PyIndexError::new_err("Index out of range"));
        }

        Ok(PyTask {
            client: self.client.clone_ref(py),
            id: self.ids[index].clone(),
        })
    }
}
