use std::sync::Arc;

use arrow::datatypes::Schema as ArrowSchema;
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::{PyOverflowError, PyRuntimeError, PyValueError};
use pyo3::types::PyAnyMethods as _;
use pyo3::{Bound, Py, PyAny, PyRef, PyRefMut, PyResult, Python, pyclass, pymethods};
use re_chunk_store::LazyStore;
use re_datafusion::{DatasetManifestProvider, SegmentTableProvider};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{DatasetDetails, DatasetEntry, EntryDetails};
use re_protos::common::v1alpha1::ext::{DatasetHandle, IfDuplicateBehavior, SegmentId};
use re_redap_client::SegmentChunkProvider;
use re_sorbet::SorbetColumnDescriptors;
use re_types_core::LayerName;

use super::registration_handle::PyRegistrationHandleInternal;
use super::{PyCatalogClientInternal, PyEntryDetails, PyTableProviderAdapterInternal, to_py_err};
use crate::catalog::PySchemaInternal;
use crate::catalog::entry::set_entry_name;
use crate::chunk_stream::lazy_store::PyLazyStoreInternal;
use crate::trace_context::read_trace_context_from_python;
use crate::utils::{get_tokio_runtime, wait_for_future};

/// A dataset entry in the catalog.
#[pyclass(
    name = "DatasetEntryInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyDatasetEntryInternal {
    client: Py<PyCatalogClientInternal>,
    entry_details: EntryDetails,
    dataset_details: DatasetDetails,
    dataset_handle: DatasetHandle,
}

impl PyDatasetEntryInternal {
    pub fn new(client: Py<PyCatalogClientInternal>, dataset_entry: DatasetEntry) -> Self {
        Self {
            client,
            entry_details: dataset_entry.details,
            dataset_details: dataset_entry.dataset_details,
            dataset_handle: dataset_entry.handle,
        }
    }

    pub fn client(&self) -> &Py<PyCatalogClientInternal> {
        &self.client
    }

    pub fn entry_id(&self) -> EntryId {
        self.entry_details.id
    }
}

#[pymethods]
impl PyDatasetEntryInternal {
    //
    // Entry methods
    //

    fn catalog(&self, py: Python<'_>) -> Py<PyCatalogClientInternal> {
        self.client.clone_ref(py)
    }

    fn entry_details(&self, py: Python<'_>) -> PyResult<Py<PyEntryDetails>> {
        Py::new(py, PyEntryDetails(self.entry_details.clone()))
    }

    /// Delete this entry from the catalog.
    fn delete(&mut self, py: Python<'_>) -> PyResult<()> {
        let _span = read_trace_context_from_python(py, "DatasetEntry.delete").entered();
        let connection = self.client.borrow_mut(py).connection().clone();
        connection.delete_entry(py, self.entry_details.id)
    }

    fn set_name(&mut self, py: Python<'_>, name: String) -> PyResult<()> {
        let _span = read_trace_context_from_python(py, "DatasetEntry.set_name").entered();
        set_entry_name(py, name, &mut self.entry_details, &self.client)
    }

    //
    // Dataset entry methods
    //

    /// Return the dataset manifest URL.
    //TODO(ab): not sure we want this to be public
    #[getter]
    fn manifest_url(&self) -> String {
        self.dataset_handle.url.to_string()
    }

    /// Return the Arrow schema of the data contained in the dataset.
    fn arrow_schema(self_: PyRef<'_, Self>) -> PyResult<PyArrowType<ArrowSchema>> {
        let _span =
            read_trace_context_from_python(self_.py(), "DatasetEntry.arrow_schema").entered();
        let arrow_schema = Self::fetch_arrow_schema(&self_)?;

        Ok(arrow_schema.into())
    }

    /// The associated blueprint dataset, if any.
    fn blueprint_dataset(self_: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Option<Py<Self>>> {
        let _span = read_trace_context_from_python(py, "DatasetEntry.blueprint_dataset").entered();
        let Some(blueprint_dataset_entry_id) = self_.dataset_details.blueprint_dataset else {
            return Ok(None);
        };

        let client = self_.client.clone_ref(py);
        let connection = self_.client.borrow(py).connection().clone();

        let dataset_entry = connection.read_dataset(py, blueprint_dataset_entry_id)?;

        Some(Py::new(py, Self::new(client, dataset_entry))).transpose()
    }

    /// The default blueprint segment ID for this dataset, if any.
    fn default_blueprint_segment_id(self_: PyRef<'_, Self>) -> Option<String> {
        self_
            .dataset_details
            .default_blueprint_segment
            .as_ref()
            .map(ToString::to_string)
    }

    /// Set the default blueprint segment ID for this dataset.
    ///
    /// Pass `None` to clear the bluprint. This fails if the change cannot be made to the remote server.
    #[pyo3(signature = (segment_id))]
    fn set_default_blueprint_segment_id(
        mut self_: PyRefMut<'_, Self>,
        py: Python<'_>,
        segment_id: Option<String>,
    ) -> PyResult<()> {
        let _span =
            read_trace_context_from_python(py, "DatasetEntry.set_default_blueprint_segment_id")
                .entered();
        let connection = self_.client.borrow(py).connection().clone();

        let mut dataset_details = self_.dataset_details.clone();
        dataset_details.default_blueprint_segment = segment_id.map(Into::into);

        let result = connection.update_dataset(py, self_.entry_details.id, dataset_details)?;

        self_.dataset_details = result.dataset_details;

        Ok(())
    }

    /// Return the schema of the data contained in the dataset.
    fn schema(self_: PyRef<'_, Self>) -> PyResult<PySchemaInternal> {
        let _span = read_trace_context_from_python(self_.py(), "DatasetEntry.schema").entered();
        Self::fetch_schema(&self_)
    }

    /// Returns a list of segment IDs for the dataset.
    pub fn segment_ids(self_: PyRef<'_, Self>) -> PyResult<Vec<String>> {
        let py = self_.py();
        let _span = read_trace_context_from_python(py, "DatasetEntry.segment_ids").entered();
        let connection = self_.client.borrow(py).connection().clone();

        connection.get_dataset_segment_ids(py, self_.entry_details.id)
    }

    /// Return the segment table as a DataFusion DataFrame.
    fn segment_table(self_: PyRef<'_, Self>) -> PyResult<Bound<'_, PyAny>> {
        let py = self_.py();
        let _span = read_trace_context_from_python(py, "DatasetEntry.segment_table").entered();
        let connection = self_.client.borrow(py).connection().clone();
        let dataset_id = self_.entry_details.id;

        let provider = wait_for_future(py, async move {
            SegmentTableProvider::new(connection.client().await?, dataset_id)
                .into_provider()
                .await
                .map_err(to_py_err)
        })?;

        let table = PyTableProviderAdapterInternal::new(provider, false);

        let client = self_.client.borrow(py);
        let ctx = client.ctx(py)?;
        let ctx = ctx.bind(py);
        drop(client);

        ctx.call_method1("read_table", (table,))
    }

    /// Return the dataset manifest as a DataFusion DataFrame.
    fn manifest(self_: PyRef<'_, Self>) -> PyResult<Bound<'_, PyAny>> {
        let py = self_.py();
        let _span = read_trace_context_from_python(py, "DatasetEntry.manifest").entered();
        let connection = self_.client.borrow(py).connection().clone();
        let dataset_id = self_.entry_details.id;

        let provider = wait_for_future(py, async move {
            DatasetManifestProvider::new(connection.client().await?, dataset_id)
                .into_provider()
                .await
                .map_err(to_py_err)
        })?;

        let table = PyTableProviderAdapterInternal::new(provider, false);

        let client = self_.client.borrow(py);
        let ctx = client.ctx(py)?;
        let ctx = ctx.bind(py);
        drop(client);

        ctx.call_method1("read_table", (table,))
    }

    /// Return the URL for the given segment.
    ///
    /// Parameters
    /// ----------
    /// segment_id: str
    ///     The ID of the segment to get the URL for.
    ///
    /// timeline: str | None
    ///     The name of the timeline to display.
    ///
    /// start: int | datetime | timedelta | None
    ///     The start selected time for the segment.
    ///     Integer for ticks, datetime/nanoseconds for timestamps, or timedelta for durations.
    ///
    /// end: int | datetime | timedelta | None
    ///     The end selected time for the segment.
    ///     Integer for ticks, datetime/nanoseconds for timestamps, or timedelta for durations.
    ///     If omitted, no time range selection is emitted (only the `#when` cursor).
    ///
    /// Examples
    /// --------
    /// # With ticks
    /// >>> start_tick, end_time = 0, 10
    /// >>> dataset.segment_url("some_id", "log_tick", start_tick, end_time)
    ///
    /// # With timestamps
    /// >>> start_time, end_time = datetime.now() - timedelta(seconds=4), datetime.now()
    /// >>> dataset.segment_url("some_id", "real_time", start_time, end_time)
    ///
    /// Returns
    /// -------
    /// str
    ///     The URL for the given segment.
    ///
    #[pyo3(signature = (segment_id, timeline=None, start=None, end=None))]
    fn segment_url(
        self_: PyRef<'_, Self>,
        py: Python<'_>,
        segment_id: String,
        timeline: Option<&str>,
        start: Option<Bound<'_, PyAny>>,
        end: Option<Bound<'_, PyAny>>,
    ) -> PyResult<String> {
        let connection = self_.client.borrow(self_.py()).connection().clone();

        // Timeline with default name and no limits overrides blueprint timeline settings
        // only override if timeline is selected
        if timeline.is_none() && (start.is_some() || end.is_some()) {
            return Err(PyValueError::new_err(
                "If `start` or `end` is specified, `timeline` must also be specified.",
            ));
        }

        // Convert Python objects to typed time cells (int → sequence, datetime → timestamp)
        let start_cell = start
            .as_ref()
            .map(|s| py_object_to_time_cell(py, s))
            .transpose()?;
        let end_cell = end
            .as_ref()
            .map(|e| py_object_to_time_cell(py, e))
            .transpose()?;

        Ok(re_uri::DatasetSegmentUri {
            origin: connection.origin().clone(),
            dataset_id: self_.entry_details.id.id,
            segment_id: SegmentId::from(segment_id),
            fragment: re_uri::Fragment {
                selection: None,
                when: timeline.map(|timeline| {
                    (
                        re_chunk::TimelineName::new(timeline),
                        start_cell.unwrap_or_else(|| {
                            re_sdk::TimeCell::new(
                                re_log_types::TimeType::TimestampNs,
                                re_log_types::NonMinI64::MIN,
                            )
                        }),
                    )
                }),
                time_selection: Option::zip(end_cell, timeline).map(|(end, timeline)| {
                    let start = start_cell.unwrap_or(end);
                    re_uri::TimeSelection {
                        timeline: re_chunk::Timeline::new(timeline, start.typ()),
                        range: re_log_types::AbsoluteTimeRange::new(start.value, end.value),
                    }
                }),
            },
        }
        .to_string())
    }

    /// Register RRD URIs to the dataset and return a handle to track progress.
    ///
    /// This method initiates the registration of recordings to the dataset, and returns
    /// a handle that can be used to wait for completion or iterate over results.
    ///
    /// Parameters
    /// ----------
    /// recording_uris: list[str]
    ///     The URIs of the RRDs to register.
    ///
    /// recording_layers: list[str]
    ///     The layers to which the recordings will be registered to.
    ///     Must be the same length as `recording_uris`.
    ///
    /// on_duplicate: str
    ///     How to handle duplicate segment layers. One of "error", "ignore", or "replace".
    #[pyo3(signature = (recording_uris, recording_layers, on_duplicate))]
    #[pyo3(text_signature = "(self, /, recording_uris, recording_layers, on_duplicate)")]
    fn register(
        self_: PyRef<'_, Self>,
        recording_uris: Vec<String>,
        recording_layers: Vec<String>,
        on_duplicate: &str,
    ) -> PyResult<PyRegistrationHandleInternal> {
        let py = self_.py();
        let connection = self_.client.borrow(py).connection().clone();
        let on_duplicate = parse_on_duplicate(on_duplicate)?;
        let _span = read_trace_context_from_python(py, "DatasetEntry.register").entered();

        let recording_layers = recording_layers.into_iter().map(LayerName::new).collect();
        let (request_trace_id, results) = connection.register_with_dataset(
            py,
            self_.entry_details.id,
            recording_uris,
            recording_layers,
            on_duplicate,
        )?;

        Ok(PyRegistrationHandleInternal::new(
            self_.client.clone_ref(py),
            results,
            request_trace_id,
        ))
    }

    /// Unregisters segments and layers from the dataset.
    ///
    /// Excluding IO errors, this will always succeed as long the target dataset exists.
    /// Corollary: unregistering data that doesn't exist is a no-op.
    ///
    /// This method acts as a *product* filter:
    /// * empty `segments_to_drop` + empty `layers_to_drop`: invalid argument error
    /// * empty `segments_to_drop` + non-empty `layers_to_drop`: remove specified layers for *all* segments
    /// * non-empty `segments_to_drop` + empty `layers_to_drop`: remove *all* layers for specified segments
    /// * non-empty `segments_to_drop` + non-empty `layers_to_drop`: delete *all* specified layers for *all* specified segments
    ///
    /// Parameters
    /// ----------
    /// segments_to_drop: list[str]
    ///     The segment IDs to drop. All of them if empty.
    ///     The final filter will be the *outer product* of this and `layers_to_drop`.
    ///
    /// layers_to_drop: list[str]
    ///     The layer names to drop. All of them if empty.
    ///     The final filter will be the *outer product* of this and `segments_to_drop`.
    ///
    /// force: bool
    ///     If true, deletion will go through regardless of the segments/layers' current statuses.
    ///     This is only useful in the very specific, catatrophic scenario where the contents of the
    ///     task queue were lost and some tasks are now stuck in `status=pending` forever.
    ///     Do not use this unless you know exactly what you're doing.
    //
    // NOTE: I'm purposefully making both parameters explicit and without default values. Deletion
    // is a scary thing, end users should have to type every character of it.
    #[pyo3(signature = (*, segments_to_drop, layers_to_drop, force=false))]
    fn unregister(
        self_: PyRef<'_, Self>,
        segments_to_drop: Vec<String>,
        layers_to_drop: Vec<String>,
        force: bool,
    ) -> PyResult<()> {
        let py = self_.py();
        let _span = read_trace_context_from_python(py, "DatasetEntry.unregister").entered();
        let connection = self_.client.borrow(py).connection().clone();

        let segments_to_drop = segments_to_drop.into_iter().map(SegmentId::new).collect();
        let layers_to_drop = layers_to_drop.into_iter().map(LayerName::new).collect();
        let _results = connection.unregister_from_dataset(
            py,
            self_.entry_details.id,
            segments_to_drop,
            layers_to_drop,
            force,
        )?;

        Ok(())
    }

    /// Register all RRDs under a given prefix to the dataset and return a handle to the tasks.
    ///
    /// A prefix is a directory-like path in an object store (e.g. an S3 bucket or ABS container).
    /// All RRDs that are recursively found under the given prefix will be registered to the dataset.
    ///
    /// This method initiates the registration of the recordings to the dataset, and returns
    /// a handle that can be used to wait for completion or iterate over results.
    ///
    /// Parameters
    /// ----------
    /// recordings_prefix: str
    ///     The prefix under which to register all RRDs.
    ///
    /// layer_name: str
    ///     The layer to which the recordings will be registered to.
    ///
    /// on_duplicate: str
    ///     How to handle duplicate segment layers. One of "error", "ignore", or "replace".
    #[pyo3(signature = (recordings_prefix, layer_name, on_duplicate))]
    #[pyo3(text_signature = "(self, /, recordings_prefix, layer_name, on_duplicate)")]
    fn register_prefix(
        self_: PyRef<'_, Self>,
        recordings_prefix: String,
        layer_name: String,
        on_duplicate: &str,
    ) -> PyResult<PyRegistrationHandleInternal> {
        let py = self_.py();
        let _span = read_trace_context_from_python(py, "DatasetEntry.register_prefix").entered();

        let connection = self_.client.borrow(py).connection().clone();
        let on_duplicate = parse_on_duplicate(on_duplicate)?;

        let (request_trace_id, results) = connection.register_with_dataset_prefix(
            py,
            self_.entry_details.id,
            recordings_prefix,
            LayerName::new(layer_name),
            on_duplicate,
        )?;

        Ok(PyRegistrationHandleInternal::new(
            self_.client.clone_ref(py),
            results,
            request_trace_id,
        ))
    }

    /// Register a single RRD URI as an asset layer (shared across all segments in the dataset).
    ///
    /// Unlike segment layers (one recording per segment), an asset layer is a single recording
    /// that is shared across all segments. This is useful for deduplicating common assets such
    /// as robot URDFs or environment meshes.
    ///
    /// !!! warning
    ///     This is an incomplete, experimental API and may change or be removed in future versions without
    ///     going through the normal deprecation cycle.
    ///
    /// Parameters
    /// ----------
    /// layer_name: str
    ///     The name of the asset layer.
    ///
    /// recording_uri: str
    ///     The URI of the RRD recording to register as the asset.
    ///
    /// on_duplicate: str
    ///     How to handle the case where the layer already exists. One of "error", "skip", or "replace".
    #[pyo3(name = "_register_asset_layer")]
    #[pyo3(signature = (*, layer_name, recording_uri, on_duplicate))]
    #[pyo3(text_signature = "(self, /, *, layer_name, recording_uri, on_duplicate)")]
    fn register_asset_layer(
        self_: PyRef<'_, Self>,
        layer_name: String,
        recording_uri: String,
        on_duplicate: &str,
    ) -> PyResult<PyRegistrationHandleInternal> {
        let py = self_.py();
        let _span =
            // TODO(RR-4797): remove experimental status
            read_trace_context_from_python(py, "DatasetEntry._register_asset_layer").entered();
        let connection = self_.client.borrow(py).connection().clone();
        let on_duplicate = parse_on_duplicate(on_duplicate)?;

        let (request_trace_id, results) = connection.register_asset_layer(
            py,
            self_.entry_details.id,
            recording_uri,
            LayerName::new(layer_name),
            on_duplicate,
        )?;

        Ok(PyRegistrationHandleInternal::new(
            self_.client.clone_ref(py),
            results,
            request_trace_id,
        ))
    }

    /// Open a remote segment as a [`LazyStore`][rerun.experimental.LazyStore].
    ///
    /// One round-trip on construction (the manifest); chunks are fetched on
    /// demand.
    fn segment_store(self_: PyRef<'_, Self>, segment_id: String) -> PyResult<PyLazyStoreInternal> {
        let py = self_.py();
        let _span = read_trace_context_from_python(py, "DatasetEntry.segment_store").entered();
        let connection = self_.client.borrow(py).connection().clone();
        let dataset_id = self_.entry_details.id;
        let segment_id = SegmentId::from(segment_id);

        let provider = wait_for_future(py, async {
            SegmentChunkProvider::try_new(
                get_tokio_runtime().handle().clone(),
                connection.connection_registry().clone(),
                connection.origin().clone(),
                dataset_id,
                segment_id,
            )
            .await
            .map_err(to_py_err)
        })?;

        let lazy = LazyStore::new(Arc::new(provider));
        Ok(PyLazyStoreInternal::new(lazy))
    }

    /// Perform maintenance tasks on the datasets.
    #[pyo3(signature = (
            optimize_indexes = false,
            retrain_indexes = false,
            compact_fragments = false,
            cleanup_before = None,
            unsafe_allow_recent_cleanup = false,
    ))]
    #[expect(clippy::fn_params_excessive_bools)]
    fn do_maintenance(
        self_: PyRef<'_, Self>,
        py: Python<'_>,
        optimize_indexes: bool,
        retrain_indexes: bool,
        compact_fragments: bool,
        cleanup_before: Option<Bound<'_, PyAny>>,
        unsafe_allow_recent_cleanup: bool,
    ) -> PyResult<()> {
        let _span = read_trace_context_from_python(py, "DatasetEntry.do_maintenance").entered();
        let connection = self_.client.borrow(self_.py()).connection().clone();

        let cleanup_before_nanos = cleanup_before
            .as_ref()
            .map(|s| py_object_to_i64(py, s))
            .transpose()?;

        let cleanup_before = cleanup_before_nanos
            .map(|ts_nanos| {
                jiff::Timestamp::from_nanosecond(ts_nanos as i128).map_err(|err| {
                    PyRuntimeError::new_err(format!(
                        "failed converting cleanup_before timestamp: {err}"
                    ))
                })
            })
            .transpose()?;

        connection.do_maintenance(
            py,
            self_.entry_details.id,
            optimize_indexes,
            retrain_indexes,
            compact_fragments,
            cleanup_before,
            unsafe_allow_recent_cleanup,
        )
    }

    /// Returns a new `DatasetView` filtered to the given segment IDs.
    ///
    /// Parameters
    /// ----------
    /// segment_ids : list[str]
    ///     A list of segment ID strings to filter to.
    ///
    /// Returns
    /// -------
    /// DatasetViewInternal
    ///     A new view filtered to the given segments.
    fn filter_segments(
        self_: PyRef<'_, Self>,
        segment_ids: Vec<String>,
    ) -> super::PyDatasetViewInternal {
        let filter: std::collections::HashSet<String> = segment_ids.into_iter().collect();
        super::PyDatasetViewInternal::new(Py::from(self_), Some(filter), None)
    }

    /// Returns a new `DatasetView` filtered to the given entity paths.
    ///
    /// Parameters
    /// ----------
    /// exprs : list[str]
    ///     Entity path expressions like `"/points/**"`, `"-/text/**"`.
    ///
    /// Returns
    /// -------
    /// DatasetViewInternal
    ///     A new view filtered to the given entity paths.
    fn filter_contents(self_: PyRef<'_, Self>, exprs: Vec<String>) -> super::PyDatasetViewInternal {
        super::PyDatasetViewInternal::new(Py::from(self_), None, Some(exprs))
    }

    pub fn __str__(self_: PyRef<'_, Self>) -> String {
        format!(
            "DatasetEntry(name='{}', id='{}')",
            self_.entry_details.name, self_.entry_details.id,
        )
    }
}

impl PyDatasetEntryInternal {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn fetch_arrow_schema(self_: &PyRef<'_, Self>) -> PyResult<ArrowSchema> {
        let connection = self_.client.borrow_mut(self_.py()).connection().clone();

        let schema = connection.get_dataset_schema(self_.py(), self_.entry_details.id)?;

        Ok(schema)
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn fetch_schema(self_: &PyRef<'_, Self>) -> PyResult<PySchemaInternal> {
        let arrow_schema = Self::fetch_arrow_schema(self_)?;
        let columns = SorbetColumnDescriptors::try_from_arrow_fields(None, arrow_schema.fields())
            .map_err(to_py_err)?;

        Ok(PySchemaInternal {
            columns,
            metadata: arrow_schema.metadata,
        })
    }
}

/// Parse the Python `on_duplicate` string into the corresponding `IfDuplicateBehavior` enum.
fn parse_on_duplicate(on_duplicate: &str) -> PyResult<IfDuplicateBehavior> {
    match on_duplicate {
        "error" => Ok(IfDuplicateBehavior::Error),
        "skip" => Ok(IfDuplicateBehavior::Skip),
        "replace" => Ok(IfDuplicateBehavior::Overwrite),
        _ => Err(PyValueError::new_err(format!(
            "invalid on_duplicate value: '{on_duplicate}'. Expected 'error', 'skip', or 'replace'"
        ))),
    }
}

/// Helper function to convert a Python object to i64.
///
/// This function attempts to convert various Python types to i64, including:
/// - Python int
/// - numpy datetime64 (via timestamp conversion)
/// - Any object with an `__int__` method
/// - Any object that can be converted to int via Python's `int()` function
fn py_object_to_i64(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<i64> {
    // First try direct extraction as i64
    if let Ok(value) = obj.extract::<i64>() {
        return Ok(value);
    }

    // Try to extract as Python int first
    if let Ok(value) = obj.extract::<i32>() {
        return Ok(value as i64);
    }

    // Check if it's a numpy datetime64 and try to get timestamp
    if obj.hasattr("timestamp")? {
        let timestamp = obj.call_method0("timestamp")?;
        if let Ok(ts_float) = timestamp.extract::<f64>() {
            // Convert seconds to nanoseconds (assuming timestamp is in seconds)
            return Ok((ts_float * 1_000_000_000.0) as i64);
        }
    }

    // Try calling __int__ method if it exists
    if obj.hasattr("__int__")? {
        let int_result = obj.call_method0("__int__")?;
        return int_result.extract::<i64>();
    }

    // As a last resort, try to convert via Python's int() function
    let int_builtin = py.import("builtins")?.getattr("int")?;
    let converted = int_builtin.call1((obj,))?;
    converted.extract::<i64>()
}

/// Convert a Python object to a [`re_sdk::TimeCell`], inferring the time type from the Python type.
///
/// Plain `int` → [`TimeType::Sequence`]; `datetime.timedelta` → [`TimeType::DurationNs`];
/// anything else (datetime, `numpy.datetime64`, …) → [`TimeType::TimestampNs`] via
/// [`py_object_to_i64`].
fn py_object_to_time_cell(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<re_sdk::TimeCell> {
    use re_log_types::TimeType;

    if let Ok(value) = obj.extract::<i64>() {
        return Ok(re_sdk::TimeCell::new(TimeType::Sequence, value));
    }

    if let Ok(duration) = obj.extract::<chrono::Duration>() {
        let nanos = duration.num_nanoseconds().ok_or_else(|| {
            PyOverflowError::new_err("datetime.timedelta is out of nanosecond range")
        })?;

        return Ok(re_sdk::TimeCell::new(TimeType::DurationNs, nanos));
    }

    let nanos = py_object_to_i64(py, obj)?;
    Ok(re_sdk::TimeCell::new(TimeType::TimestampNs, nanos))
}
