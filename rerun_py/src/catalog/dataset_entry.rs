use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchOptions, StringArray};
use arrow::datatypes::{Field, Schema as ArrowSchema};
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::types::PyAnyMethods as _;
use pyo3::{Bound, Py, PyAny, PyErr, PyRef, PyRefMut, PyResult, Python, pyclass, pymethods};
use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_datafusion::{DatasetManifestProvider, SearchResultsTableProvider, SegmentTableProvider};
use re_log_types::{EntryId, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::ext::{
    DatasetDetails, DatasetEntry, EntryDetails, IndexProperties,
};
use re_protos::cloud::v1alpha1::{
    CreateIndexRequest, DeleteIndexesRequest, IndexConfig, IndexQueryProperties,
    InvertedIndexQuery, ListIndexesRequest, SearchDatasetRequest, VectorIndexQuery,
    index_query_properties,
};
use re_protos::common::v1alpha1::ext::DatasetHandle;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::fetch_chunks_response_to_chunk_and_segment_id;
use re_sorbet::{SorbetColumnDescriptors, TimeColumnSelector};
use tokio_stream::StreamExt as _;
use tracing::instrument;

use super::registration_handle::PyRegistrationHandleInternal;
use super::{
    PyCatalogClientInternal, PyEntryDetails, PyIndexConfig, PyIndexingResult,
    PyTableProviderAdapterInternal, VectorDistanceMetricLike, VectorLike, to_py_err,
};
use crate::catalog::entry::set_entry_name;
use crate::catalog::{AnyComponentColumn, PyIndexColumnSelector, PySchemaInternal};
use crate::recording::PyRecording;
use crate::utils::wait_for_future;

/// A dataset entry in the catalog.
#[pyclass( // NOLINT: ignore[py-cls-eq] non-trivial implementation
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
        let connection = self.client.borrow_mut(py).connection().clone();
        connection.delete_entry(py, self.entry_details.id)
    }

    fn set_name(&mut self, py: Python<'_>, name: String) -> PyResult<()> {
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
    //TODO(#9457): there should be another `schema` method which returns a `PySchema`
    #[instrument(skip_all)]
    fn arrow_schema(self_: PyRef<'_, Self>) -> PyResult<PyArrowType<ArrowSchema>> {
        let arrow_schema = Self::fetch_arrow_schema(&self_)?;

        Ok(arrow_schema.into())
    }

    /// The associated blueprint dataset, if any.
    fn blueprint_dataset(self_: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Option<Py<Self>>> {
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
        let connection = self_.client.borrow(py).connection().clone();

        let mut dataset_details = self_.dataset_details.clone();
        dataset_details.default_blueprint_segment = segment_id.map(Into::into);

        let result = connection.update_dataset(py, self_.entry_details.id, dataset_details)?;

        self_.dataset_details = result.dataset_details;

        Ok(())
    }

    /// Return the schema of the data contained in the dataset.
    fn schema(self_: PyRef<'_, Self>) -> PyResult<PySchemaInternal> {
        Self::fetch_schema(&self_)
    }

    /// Returns a list of segment IDs for the dataset.
    pub fn segment_ids(self_: PyRef<'_, Self>) -> PyResult<Vec<String>> {
        let connection = self_.client.borrow(self_.py()).connection().clone();

        connection.get_dataset_segment_ids(self_.py(), self_.entry_details.id)
    }

    /// Return the segment table as a DataFusion DataFrame.
    #[instrument(skip_all)]
    fn segment_table(self_: PyRef<'_, Self>) -> PyResult<Bound<'_, PyAny>> {
        let py = self_.py();
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
    #[instrument(skip_all)]
    fn manifest(self_: PyRef<'_, Self>) -> PyResult<Bound<'_, PyAny>> {
        let py = self_.py();
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
    /// start: int | datetime | None
    ///     The start selected time for the segment.
    ///     Integer for ticks, or datetime/nanoseconds for timestamps.
    ///
    /// end: int | datetime | None
    ///     The end selected time for the segment.
    ///     Integer for ticks, or datetime/nanoseconds for timestamps.
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

        // Convert Python objects to i64
        let start_i64 = start
            .as_ref()
            .map(|s| py_object_to_i64(py, s))
            .transpose()?;
        let end_i64 = end.as_ref().map(|e| py_object_to_i64(py, e)).transpose()?;

        Ok(re_uri::DatasetSegmentUri {
            origin: connection.origin().clone(),
            dataset_id: self_.entry_details.id.id,
            segment_id,

            //TODO(ab): add support for this
            fragment: re_uri::Fragment {
                selection: None,
                when: timeline.map(|timeline| {
                    (
                        re_chunk::TimelineName::new(timeline),
                        re_sdk::TimeCell::new(
                            re_log_types::TimeType::TimestampNs,
                            start_i64
                                .map(|start| start.try_into().expect("start time must be valid"))
                                .unwrap_or(re_log_types::NonMinI64::MIN),
                        ),
                    )
                }),
                time_selection: timeline.map(|timeline| re_uri::TimeSelection {
                    timeline: re_chunk::Timeline::new_timestamp(timeline),
                    range: re_log_types::AbsoluteTimeRange::new(
                        start_i64
                            .map(|start| start.try_into().expect("start time must be valid"))
                            .unwrap_or(re_log_types::NonMinI64::MIN),
                        end_i64
                            .map(|end| end.try_into().expect("end time must be valid"))
                            .unwrap_or(re_log_types::NonMinI64::MAX),
                    ),
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
    #[pyo3(signature = (recording_uris, *, recording_layers))]
    #[pyo3(text_signature = "(self, /, recording_uris, *, recording_layers)")]
    fn register(
        self_: PyRef<'_, Self>,
        recording_uris: Vec<String>,
        recording_layers: Vec<String>,
    ) -> PyResult<PyRegistrationHandleInternal> {
        let connection = self_.client.borrow(self_.py()).connection().clone();

        let results = connection.register_with_dataset(
            self_.py(),
            self_.entry_details.id,
            recording_uris,
            recording_layers,
        )?;

        Ok(PyRegistrationHandleInternal::new(
            self_.client.clone_ref(self_.py()),
            results,
        ))
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
    /// layer_name: Optional[str]
    ///     The layer to which the recordings will be registered to.
    ///     If `None`, this defaults to `"base"`.
    #[pyo3(signature = (recordings_prefix, layer_name = None))]
    #[pyo3(text_signature = "(self, /, recordings_prefix, layer_name = None)")]
    fn register_prefix(
        self_: PyRef<'_, Self>,
        recordings_prefix: String,
        layer_name: Option<String>,
    ) -> PyResult<PyRegistrationHandleInternal> {
        let connection = self_.client.borrow(self_.py()).connection().clone();

        let results = connection.register_with_dataset_prefix(
            self_.py(),
            self_.entry_details.id,
            recordings_prefix,
            layer_name,
        )?;

        Ok(PyRegistrationHandleInternal::new(
            self_.client.clone_ref(self_.py()),
            results,
        ))
    }

    /// Download a segment from the dataset.
    #[instrument(skip(self_), err)]
    fn download_segment(self_: PyRef<'_, Self>, segment_id: String) -> PyResult<PyRecording> {
        let catalog_client = self_.client.borrow(self_.py());
        let connection = catalog_client.connection();
        let dataset_id = self_.entry_details.id;
        let dataset_name = self_.entry_details.name.clone();

        let store: PyResult<ChunkStore> = wait_for_future(self_.py(), async move {
            let mut client = connection.client().await?;
            let response_stream = client
                .fetch_segment_chunks_by_query(re_redap_client::SegmentQueryParams {
                    dataset_id,
                    segment_id: segment_id.clone().into(),
                    include_static_data: true,
                    include_temporal_data: true,
                    query: None,
                })
                .await
                .map_err(to_py_err)?;

            let mut chunks_stream = fetch_chunks_response_to_chunk_and_segment_id(response_stream);

            let store_id = StoreId::new(StoreKind::Recording, dataset_name, segment_id.clone());
            let mut store = ChunkStore::new(store_id, Default::default());

            while let Some(chunks) = chunks_stream.next().await {
                for chunk in chunks.map_err(to_py_err)? {
                    let (chunk, chunk_segment_id) = chunk;

                    if Some(&segment_id) != chunk_segment_id.as_ref() {
                        re_log::warn!(
                            expected = segment_id,
                            got = chunk_segment_id,
                            "unexpected segment ID in chunk stream, this is a bug"
                        );
                    }
                    store
                        .insert_chunk(&std::sync::Arc::new(chunk))
                        .map_err(to_py_err)?;
                }
            }

            Ok(store)
        });

        let handle = ChunkStoreHandle::new(store?);

        Ok(PyRecording { store: handle })
    }

    // TODO(RR-2824): we should have a generic `create_index(PyIndexConfig)`

    /// Create a full-text search index on the given column.
    #[pyo3(signature = (
            *,
            column,
            time_index,
            store_position = false,
            base_tokenizer = "simple",
        ))]
    #[instrument(skip(self_, column, time_index), err)]
    fn create_fts_search_index(
        self_: PyRef<'_, Self>,
        column: AnyComponentColumn,
        time_index: PyIndexColumnSelector,
        store_position: bool,
        base_tokenizer: &str,
    ) -> PyResult<()> {
        let connection = self_.client.borrow(self_.py()).connection().clone();
        let dataset_id = self_.entry_details.id;
        let time_selector: TimeColumnSelector = time_index.into();

        let schema = Self::fetch_schema(&self_)?;
        let component_descriptor = schema.column_for_selector(column)?;

        let properties = IndexProperties::Inverted {
            store_position,
            base_tokenizer: base_tokenizer.into(),
        };

        let request = CreateIndexRequest {
            config: Some(IndexConfig {
                properties: Some(properties.into()),
                column: Some(component_descriptor.0.into()),
                time_index: Some(time_selector.timeline.into()),
            }),
        };

        wait_for_future(self_.py(), async {
            connection
                .client()
                .await?
                .inner()
                .create_index(
                    tonic::Request::new(request)
                        .with_entry_id(dataset_id)
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
                )
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            Ok(())
        })
    }

    /// Create a vector index on the given column.
    ///
    /// This will enable indexing and build the vector index over all existing values
    /// in the specified component column.
    ///
    /// Results can be retrieved using the `search_vector` API, which will include
    /// the time-point on the indexed timeline.
    ///
    /// Only one index can be created per component column -- executing this a second
    /// time for the same component column will replace the existing index.
    ///
    /// Parameters
    /// ----------
    /// column : AnyComponentColumn
    ///     The component column to create the index on.
    /// time_index : IndexColumnSelector
    ///     Which timeline this index will map to.
    /// target_partition_num_rows : int | None
    ///     The target size (in number of rows) for each partition.
    ///     The underlying indexer (lance) will pick a default when no value
    ///     is specified - today this is 8192. It will also cap the
    ///     maximum number of partitions independently of this setting - currently
    ///     4096.
    /// num_sub_vectors : int
    ///     The number of sub-vectors to use when building the index.
    /// distance_metric : VectorDistanceMetricLike
    ///     The distance metric to use for the index. ("L2", "Cosine", "Dot", "Hamming")
    #[pyo3(signature = (
        *,
        column,
        time_index,
        target_partition_num_rows = None,
        num_sub_vectors = 16,
        distance_metric = VectorDistanceMetricLike::VectorDistanceMetric(crate::catalog::PyVectorDistanceMetric::Cosine),
    ))]
    #[instrument(skip(self_, column, time_index, distance_metric), err)]
    fn create_vector_search_index(
        self_: PyRef<'_, Self>,
        column: AnyComponentColumn,
        time_index: PyIndexColumnSelector,
        target_partition_num_rows: Option<u32>,
        num_sub_vectors: u32,
        distance_metric: VectorDistanceMetricLike,
    ) -> PyResult<PyIndexingResult> {
        let connection = self_.client.borrow(self_.py()).connection().clone();
        let dataset_id = self_.entry_details.id;

        let time_selector: TimeColumnSelector = time_index.into();

        let schema = Self::fetch_schema(&self_)?;
        let component_descriptor = schema.column_for_selector(column)?;

        let distance_metric: re_protos::cloud::v1alpha1::VectorDistanceMetric =
            distance_metric.try_into()?;

        let properties = IndexProperties::VectorIvfPq {
            target_partition_num_rows,
            num_sub_vectors,
            metric: distance_metric,
        };

        let config = re_protos::cloud::v1alpha1::ext::IndexConfig {
            time_index: time_selector.timeline,
            column: component_descriptor.0.clone().into(),
            properties: properties.clone(),
        };

        let request = CreateIndexRequest {
            config: Some(IndexConfig {
                properties: Some(properties.into()),
                column: Some(component_descriptor.0.into()),
                time_index: Some(time_selector.timeline.into()),
            }),
        };

        wait_for_future(self_.py(), async {
            let result = connection
                .client()
                .await?
                .inner()
                .create_index(
                    tonic::Request::new(request)
                        .with_entry_id(dataset_id)
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
                )
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner();

            Ok(PyIndexingResult {
                index: config.into(),
                statistics_json: result.statistics_json,
                debug_info: result.debug_info,
            })
        })
    }

    /// List all user-defined indexes in this dataset.
    #[instrument(skip_all, err)]
    fn list_search_indexes(self_: PyRef<'_, Self>) -> PyResult<Vec<PyIndexingResult>> {
        let connection = self_.client.borrow(self_.py()).connection().clone();
        let dataset_id = self_.entry_details.id;

        let request = ListIndexesRequest {};

        wait_for_future(self_.py(), async {
            let result = connection
                .client()
                .await?
                .inner()
                .list_indexes(
                    tonic::Request::new(request)
                        .with_entry_id(dataset_id)
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
                )
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner();

            let indexes: Result<Vec<_>, PyErr> = result
                .indexes
                .into_iter()
                .map(|index| {
                    let index = re_protos::cloud::v1alpha1::ext::IndexConfig::try_from(index)?;
                    Ok(PyIndexConfig::from(index))
                })
                .collect();

            Ok(itertools::izip!(indexes?, result.statistics_json)
                .map(|(index, statistics_json)| PyIndexingResult {
                    index,
                    statistics_json,
                    debug_info: None,
                })
                .collect())
        })
    }

    /// Deletes all user-defined indexes for the specified column.
    //
    // TODO(RR-2824): this should also be capable of accepting a `PyIndexConfig` directly.
    #[instrument(skip_all, err)]
    fn delete_search_indexes(
        self_: PyRef<'_, Self>,
        column: AnyComponentColumn,
    ) -> PyResult<Vec<PyIndexConfig>> {
        let connection = self_.client.borrow(self_.py()).connection().clone();
        let dataset_id = self_.entry_details.id;

        let schema = Self::fetch_schema(&self_)?;
        let component_descriptor = schema.column_for_selector(column)?;

        let request = DeleteIndexesRequest {
            column: Some(component_descriptor.0.into()),
        };

        wait_for_future(self_.py(), async {
            let result = connection
                .client()
                .await?
                .inner()
                .delete_indexes(
                    tonic::Request::new(request)
                        .with_entry_id(dataset_id)
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
                )
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner();

            let indexes: Result<Vec<_>, PyErr> = result
                .indexes
                .into_iter()
                .map(|index| {
                    let index = re_protos::cloud::v1alpha1::ext::IndexConfig::try_from(index)?;
                    Ok(PyIndexConfig::from(index))
                })
                .collect();

            indexes
        })
    }

    /// Search the dataset using a full-text search query.
    #[instrument(skip(self_, column), err)]
    fn search_fts(
        self_: PyRef<'_, Self>,
        query: String,
        column: AnyComponentColumn,
    ) -> PyResult<Bound<'_, PyAny>> {
        let py = self_.py();
        let connection = self_.client.borrow(py).connection().clone();
        let dataset_id = self_.entry_details.id;

        let schema = Self::fetch_schema(&self_)?;
        let component_descriptor = schema.column_for_selector(column)?;

        let schema = arrow::datatypes::Schema::new_with_metadata(
            vec![Field::new("items", arrow::datatypes::DataType::Utf8, false)],
            Default::default(),
        );

        let query = RecordBatch::try_new_with_options(
            Arc::new(schema),
            vec![Arc::new(StringArray::from_iter_values([query]))],
            &RecordBatchOptions::default().with_row_count(Some(1)),
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        let request = SearchDatasetRequest {
            column: Some(component_descriptor.0.into()),
            properties: Some(IndexQueryProperties {
                props: Some(
                    re_protos::cloud::v1alpha1::index_query_properties::Props::Inverted(
                        InvertedIndexQuery {},
                    ),
                ),
            }),
            query: Some(query.into()),
            scan_parameters: None,
        };

        let provider = wait_for_future(py, async move {
            SearchResultsTableProvider::new(connection.client().await?, dataset_id, request)
                .map_err(to_py_err)?
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

    /// Search the dataset using a vector search query.
    #[instrument(skip(self_, query, column), err)]
    fn search_vector<'py>(
        self_: PyRef<'py, Self>,
        query: VectorLike<'_>,
        column: AnyComponentColumn,
        top_k: u32,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let connection = self_.client.borrow(py).connection().clone();
        let dataset_id = self_.entry_details.id;

        let schema = Self::fetch_schema(&self_)?;
        let component_descriptor = schema.column_for_selector(column)?;

        let query = query.to_record_batch()?;

        let request = SearchDatasetRequest {
            column: Some(component_descriptor.0.into()),
            properties: Some(IndexQueryProperties {
                props: Some(index_query_properties::Props::Vector(VectorIndexQuery {
                    top_k: Some(top_k),
                })),
            }),
            query: Some(query.into()),
            scan_parameters: None,
        };

        let provider = wait_for_future(py, async move {
            SearchResultsTableProvider::new(connection.client().await?, dataset_id, request)
                .map_err(to_py_err)?
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

    /// Perform maintenance tasks on the datasets.
    #[pyo3(signature = (
            optimize_indexes = false,
            retrain_indexes = false,
            compact_fragments = false,
            cleanup_before = None,
            unsafe_allow_recent_cleanup = false,
    ))]
    #[instrument(skip_all, err)]
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
    pub fn fetch_arrow_schema(self_: &PyRef<'_, Self>) -> PyResult<ArrowSchema> {
        let connection = self_.client.borrow_mut(self_.py()).connection().clone();

        let schema = connection.get_dataset_schema(self_.py(), self_.entry_details.id)?;

        Ok(schema)
    }

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
