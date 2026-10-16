use std::collections::HashSet;

use pyo3::{Py, PyResult, Python, pyclass, pymethods};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_types_core::LayerName;

use super::{PyDatasetEntryInternal, PySchemaInternal, PyUnregistrationHandleInternal, to_py_err};
use crate::trace_context::read_trace_context_from_python;
use crate::utils::wait_for_future;

/// A handle to a segment in a dataset.
///
/// The handle keeps the dataset alive and verifies that the segment exists when created.
#[pyclass(name = "SegmentInternal", module = "rerun_bindings.rerun_bindings")]
pub struct PySegmentInternal {
    dataset: Py<PyDatasetEntryInternal>,
    segment_id: SegmentId,
}

impl PySegmentInternal {
    pub fn new(
        py: Python<'_>,
        dataset: Py<PyDatasetEntryInternal>,
        segment_id: SegmentId,
    ) -> PyResult<Self> {
        let (connection, dataset_id) = {
            let dataset = dataset.borrow(py);
            (
                dataset.client().borrow(py).connection().clone(),
                dataset.entry_id(),
            )
        };
        wait_for_future(py, async {
            connection
                .inner()
                .ensure_segment_exists(dataset_id, segment_id.clone())
                .await
                .map_err(to_py_err)
        })?;
        Ok(Self {
            dataset,
            segment_id,
        })
    }
}

#[pymethods]
impl PySegmentInternal {
    /// The segment ID within its dataset.
    #[getter]
    fn id(&self) -> String {
        self.segment_id.to_string()
    }

    /// The dataset entry that owns this segment.
    #[getter]
    fn dataset(&self, py: Python<'_>) -> Py<PyDatasetEntryInternal> {
        self.dataset.clone_ref(py)
    }

    /// The URL for this segment.
    #[getter]
    fn url(&self, py: Python<'_>) -> String {
        let dataset = self.dataset.borrow(py);
        let client = dataset.client().borrow(py);
        re_uri::DatasetUri {
            origin: client.connection().origin().clone(),
            dataset_id: dataset.entry_id().id,
            resource: re_uri::DatasetResource::Segments,
            segment_id: Some(self.segment_id.clone()),
            fragment: re_uri::Fragment::default(),
        }
        .to_string()
    }

    fn __str__(&self) -> &str {
        self.segment_id.as_str()
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        format!(
            "Segment(dataset_id={}, id={:?})",
            self.dataset.borrow(py).entry_id(),
            self.segment_id.as_str()
        )
    }

    fn schema(&self, py: Python<'_>) -> PyResult<PySchemaInternal> {
        let _span = read_trace_context_from_python(py, "Segment.schema").entered();
        let Self {
            dataset,
            segment_id,
        } = self;
        let (connection, dataset_id) = {
            let dataset = dataset.borrow(py);
            (
                dataset.client().borrow(py).connection().clone(),
                dataset.entry_id(),
            )
        };
        let schema = wait_for_future(py, async {
            connection
                .inner()
                .get_segment_schema(dataset_id, segment_id.clone())
                .await
                .map_err(to_py_err)
        })?;
        PySchemaInternal::from_arrow_schema(schema)
    }

    /// Read the current layer names from the segment table.
    fn layers(&self, py: Python<'_>) -> PyResult<HashSet<String>> {
        let _span = read_trace_context_from_python(py, "Segment.layers").entered();
        let Self {
            dataset,
            segment_id,
        } = self;
        let (connection, dataset_id) = {
            let dataset = dataset.borrow(py);
            (
                dataset.client().borrow(py).connection().clone(),
                dataset.entry_id(),
            )
        };
        let layers = wait_for_future(py, async {
            connection
                .inner()
                .get_segment_layers(dataset_id, segment_id.clone())
                .await
                .map_err(to_py_err)
        })?;

        Ok(layers.into_iter().map(LayerName::into_string).collect())
    }

    /// Unregister one layer and return a handle to track completion.
    fn delete_layer(
        &self,
        py: Python<'_>,
        name: String,
    ) -> PyResult<PyUnregistrationHandleInternal> {
        let _span = read_trace_context_from_python(py, "Segment.delete_layer").entered();
        let Self {
            dataset,
            segment_id,
        } = self;
        let layer = LayerName::try_new(name).map_err(to_py_err)?;
        let (client, dataset_id) = {
            let dataset = dataset.borrow(py);
            (dataset.client().clone_ref(py), dataset.entry_id())
        };
        let connection = client.borrow(py).connection().clone();
        let (request_trace_id, tasks) = wait_for_future(py, async {
            connection
                .inner()
                .delete_segment_layer(dataset_id, segment_id.clone(), layer)
                .await
                .map_err(to_py_err)
        })?;

        Ok(PyUnregistrationHandleInternal::new(
            client,
            tasks,
            request_trace_id,
        ))
    }
}
