use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchOptions, StringArray};
use arrow::datatypes::{Field, Schema as ArrowSchema};
use arrow::pyarrow::PyArrowType;
use pyo3::types::PyAnyMethods as _;
use pyo3::{Bound, PyErr};
use pyo3::{
    Py, PyAny, PyRef, PyRefMut, PyResult, Python, exceptions::PyRuntimeError,
    exceptions::PyValueError, pyclass, pymethods,
};
use tokio_stream::StreamExt as _;
use tracing::instrument;

use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_datafusion::{DatasetManifestProvider, PartitionTableProvider, SearchResultsTableProvider};
use re_log_types::{EntryId, StoreId, StoreKind};
use re_protos::{
    cloud::v1alpha1::{
        CreateIndexRequest, DeleteIndexesRequest, IndexConfig, IndexQueryProperties,
        InvertedIndexQuery, ListIndexesRequest, SearchDatasetRequest, VectorIndexQuery,
        ext::{DatasetDetails, DatasetEntry, EntryDetails, IndexProperties},
        index_query_properties,
    },
    common::v1alpha1::ext::DatasetHandle,
    headers::RerunHeadersInjectorExt as _,
};
use re_redap_client::fetch_chunks_response_to_chunk_and_partition_id;
use re_sorbet::{SorbetColumnDescriptors, TimeColumnSelector};

use super::{
    PyCatalogClientInternal, PyDataFusionTable, PyEntryDetails, PyEntryId, PyIndexConfig,
    PyIndexingResult, VectorDistanceMetricLike, VectorLike, dataframe_query::PyDataframeQueryView,
    task::PyTasks, to_py_err,
};
use crate::catalog::entry::update_entry;
use crate::dataframe::{AnyComponentColumn, PyIndexColumnSelector, PyRecording, PySchema};
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

    #[pyo3(signature = (*, name=None))]
    fn update(&mut self, py: Python<'_>, name: Option<String>) -> PyResult<()> {
        update_entry(py, name, &mut self.entry_details, &self.client)
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

    /// The ID of the associated blueprint dataset, if any.
    fn blueprint_dataset_id(self_: PyRef<'_, Self>) -> Option<PyEntryId> {
        self_.dataset_details.blueprint_dataset.map(Into::into)
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

    /// The default blueprint partition ID for this dataset, if any.
    fn default_blueprint_partition_id(self_: PyRef<'_, Self>) -> Option<String> {
        self_
            .dataset_details
            .default_blueprint
            .as_ref()
            .map(ToString::to_string)
    }

    /// Set the default blueprint partition ID for this dataset.
    ///
    /// Pass `None` to clear the bluprint. This fails if the change cannot be made to the remote server.
    #[pyo3(signature = (partition_id))]
    fn set_default_blueprint_partition_id(
        mut self_: PyRefMut<'_, Self>,
        py: Python<'_>,
        partition_id: Option<String>,
    ) -> PyResult<()> {
        let connection = self_.client.borrow(py).connection().clone();

        let mut dataset_details = self_.dataset_details.clone();
        dataset_details.default_blueprint = partition_id.map(Into::into);

        let result = connection.update_dataset(py, self_.entry_details.id, dataset_details)?;

        self_.dataset_details = result.dataset_details;

        Ok(())
    }

    /// Return the schema of the data contained in the dataset.
    fn schema(self_: PyRef<'_, Self>) -> PyResult<PySchema> {
        Self::fetch_schema(&self_)
    }

    /// Returns a list of partitions IDs for the dataset.
    fn partition_ids(self_: PyRef<'_, Self>) -> PyResult<Vec<String>> {
        let connection = self_.client.borrow(self_.py()).connection().clone();

        connection.get_dataset_partition_ids(self_.py(), self_.entry_details.id)
    }

    /// Return the partition table as a Datafusion table provider.
    #[instrument(skip_all)]
    fn partition_table(self_: PyRef<'_, Self>) -> PyResult<PyDataFusionTable> {
        let connection = self_.client.borrow(self_.py()).connection().clone();
        let dataset_id = self_.entry_details.id;

        let provider = wait_for_future(self_.py(), async move {
            PartitionTableProvider::new(connection.client().await?, dataset_id)
                .into_provider()
                .await
                .map_err(to_py_err)
        })?;

        Ok(PyDataFusionTable {
            client: self_.client.clone_ref(self_.py()),
            name: format!("{}_partition_table", self_.entry_details.name),
            provider,
        })
    }

    /// Return the dataset manifest as a Datafusion table provider.
    #[instrument(skip_all)]
    fn manifest(self_: PyRef<'_, Self>) -> PyResult<PyDataFusionTable> {
        let connection = self_.client.borrow(self_.py()).connection().clone();
        let dataset_id = self_.entry_details.id;

        let provider = wait_for_future(self_.py(), async move {
            DatasetManifestProvider::new(connection.client().await?, dataset_id)
                .into_provider()
                .await
                .map_err(to_py_err)
        })?;

        Ok(PyDataFusionTable {
            client: self_.client.clone_ref(self_.py()),
            name: format!("{}_manifest", self_.entry_details.name),
            provider,
        })
    }

    /// Return the URL for the given partition.
    ///
    /// Parameters
    /// ----------
    /// partition_id: str
    ///     The ID of the partition to get the URL for.
    ///
    /// timeline: str | None
    ///     The name of the timeline to display.
    ///
    /// start: int | datetime | None
    ///     The start time for the partition.
    ///     Integer for ticks, or datetime/nanoseconds for timestamps.
    ///
    /// end: int | datetime | None
    ///     The end time for the partition.
    ///     Integer for ticks, or datetime/nanoseconds for timestamps.
    ///
    /// Examples
    /// --------
    /// # With ticks
    /// >>> start_tick, end_time = 0, 10
    /// >>> dataset.partition_url("some_id", "log_tick", start_tick, end_time)
    ///
    /// # With timestamps
    /// >>> start_time, end_time = datetime.now() - timedelta(seconds=4), datetime.now()
    /// >>> dataset.partition_url("some_id", "real_time", start_time, end_time)
    ///
    /// Returns
    /// -------
    /// str
    ///     The URL for the given partition.
    ///
    #[pyo3(signature = (partition_id, timeline=None, start=None, end=None))]
    fn partition_url(
        self_: PyRef<'_, Self>,
        py: Python<'_>,
        partition_id: String,
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

        let time_range: Option<re_uri::TimeSelection> =
            timeline.map(|name| re_uri::TimeSelection {
                timeline: re_chunk::Timeline::new_timestamp(name),
                range: re_log_types::AbsoluteTimeRange::new(
                    start_i64
                        .map(|start| start.try_into().expect("start time must be valid"))
                        .unwrap_or(re_log_types::NonMinI64::MIN),
                    end_i64
                        .map(|end| end.try_into().expect("end time must be valid"))
                        .unwrap_or(re_log_types::NonMinI64::MAX),
                ),
            });
        Ok(re_uri::DatasetPartitionUri {
            origin: connection.origin().clone(),
            dataset_id: self_.entry_details.id.id,
            partition_id,

            time_range,
            //TODO(ab): add support for this
            fragment: Default::default(),
        }
        .to_string())
    }

    /// Register a RRD URI to the dataset and wait for completion.
    ///
    /// This method registers a single recording to the dataset and blocks until the registration is
    /// complete, or after a timeout (in which case, a `TimeoutError` is raised).
    ///
    /// Parameters
    /// ----------
    /// recording_uri: str
    ///     The URI of the RRD to register.
    ///
    /// recording_layer: str
    ///     The layer to which the recording will be registered to.
    ///
    /// timeout_secs: int
    ///     The timeout after which this method raises a `TimeoutError` if the task is not completed.
    ///
    /// Returns
    /// -------
    /// partition_id: str
    ///     The partition ID of the registered RRD.
    #[pyo3(signature = (recording_uri, *, recording_layer = "base".to_owned(), timeout_secs = 60))]
    #[pyo3(
        text_signature = "(self, /, recording_uri, *, recording_layer = 'base', timeout_secs = 60)"
    )]
    fn register(
        self_: PyRef<'_, Self>,
        recording_uri: String,
        recording_layer: String,
        timeout_secs: u64,
    ) -> PyResult<String> {
        let register_timeout = std::time::Duration::from_secs(timeout_secs);
        let connection = self_.client.borrow(self_.py()).connection().clone();

        let mut results = connection.register_with_dataset(
            self_.py(),
            self_.entry_details.id,
            vec![recording_uri],
            vec![recording_layer],
        )?;

        let Some(task_descriptor) = results.pop() else {
            return Err(PyRuntimeError::new_err(
                "Failed to register recording, no task returned.",
            ));
        };

        connection.wait_for_tasks(self_.py(), vec![task_descriptor.task_id], register_timeout)?;

        Ok(task_descriptor.partition_id.id)
    }

    /// Register all RRDs under a given prefix to the dataset and return a handle to the tasks.
    ///
    /// A prefix is a directory-like path in an object store (e.g. an S3 bucket or ABS container).
    /// All RRDs that are recursively found under the given prefix will be registered to the dataset.
    ///
    /// This method initiates the registration of the recordings to the dataset, and returns
    /// the corresponding task ids in a [`Tasks`] object.
    ///
    /// Parameters
    /// ----------
    /// recordings_prefix: str
    ///     The prefix under which to register all RRDs.
    ///
    /// layer_name: Optional[str]
    ///     The layer to which the recordings will be registered to.
    ///     If `None`, this defaults to `"base"`.
    #[allow(clippy::allow_attributes, rustdoc::broken_intra_doc_links)]
    #[pyo3(signature = (
        recordings_prefix,
        layer_name = None,
    ))]
    #[pyo3(text_signature = "(self, /, recordings_prefix, layer_name = None)")]
    fn register_prefix(
        self_: PyRef<'_, Self>,
        recordings_prefix: String,
        layer_name: Option<String>,
    ) -> PyResult<PyTasks> {
        let connection = self_.client.borrow(self_.py()).connection().clone();

        let results = connection.register_with_dataset_prefix(
            self_.py(),
            self_.entry_details.id,
            recordings_prefix,
            layer_name,
        )?;

        Ok(PyTasks::new(
            self_.client.clone_ref(self_.py()),
            results.into_iter().map(|desc| desc.task_id),
        ))
    }

    /// Register a batch of RRD URIs to the dataset and return a handle to the tasks.
    ///
    /// This method initiates the registration of multiple recordings to the dataset, and returns
    /// the corresponding task ids in a [`Tasks`] object.
    ///
    /// Parameters
    /// ----------
    /// recording_uris: list[str]
    ///     The URIs of the RRDs to register.
    ///
    /// recording_layers: list[str]
    ///     The layers to which the recordings will be registered to:
    ///     * When empty, this defaults to `["base"]`.
    ///     * If longer than `recording_uris`, `recording_layers` will be truncated.
    ///     * If shorter than `recording_uris`, `recording_layers` will be extended by repeating its last value.
    ///       I.e. an empty `recording_layers` will result in `"base"` begin repeated `len(recording_layers)` times.
    #[allow(clippy::allow_attributes, rustdoc::broken_intra_doc_links)]
    #[pyo3(signature = (
        recording_uris,
        *,
        recording_layers = vec![],
    ))]
    #[pyo3(text_signature = "(self, /, recording_uris, *, recording_layers)")]
    // TODO(ab): it might be useful to return partition ids directly since we have them
    fn register_batch(
        self_: PyRef<'_, Self>,
        recording_uris: Vec<String>,
        recording_layers: Vec<String>,
    ) -> PyResult<PyTasks> {
        let connection = self_.client.borrow(self_.py()).connection().clone();

        let results = connection.register_with_dataset(
            self_.py(),
            self_.entry_details.id,
            recording_uris,
            recording_layers,
        )?;

        Ok(PyTasks::new(
            self_.client.clone_ref(self_.py()),
            results.into_iter().map(|desc| desc.task_id),
        ))
    }

    /// Download a partition from the dataset.
    #[instrument(skip(self_), err)]
    fn download_partition(self_: PyRef<'_, Self>, partition_id: String) -> PyResult<PyRecording> {
        let catalog_client = self_.client.borrow(self_.py());
        let connection = catalog_client.connection();
        let dataset_id = self_.entry_details.id;
        let dataset_name = self_.entry_details.name.clone();

        let store: PyResult<ChunkStore> = wait_for_future(self_.py(), async move {
            let mut client = connection.client().await?;
            let response_stream = client
                .fetch_partition_chunks_by_query(re_redap_client::PartitionQueryParams {
                    dataset_id,
                    partition_id: partition_id.clone().into(),
                    include_static_data: true,
                    include_temporal_data: true,
                    query: None,
                })
                .await
                .map_err(to_py_err)?;

            let mut chunks_stream =
                fetch_chunks_response_to_chunk_and_partition_id(response_stream);

            let store_id = StoreId::new(StoreKind::Recording, dataset_name, partition_id.clone());
            let mut store = ChunkStore::new(store_id, Default::default());

            while let Some(chunks) = chunks_stream.next().await {
                for chunk in chunks.map_err(to_py_err)? {
                    let (chunk, chunk_partition_id) = chunk;

                    if Some(&partition_id) != chunk_partition_id.as_ref() {
                        re_log::warn!(
                            expected = partition_id,
                            got = chunk_partition_id,
                            "unexpected partition ID in chunk stream, this is a bug"
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

        let cache =
            re_dataframe::QueryCacheHandle::new(re_dataframe::QueryCache::new(handle.clone()));

        Ok(PyRecording {
            store: handle,
            cache,
        })
    }

    #[allow(
        clippy::allow_attributes,
        rustdoc::private_doc_tests,
        rustdoc::invalid_rust_codeblocks
    )]
    /// Create a [`DataframeQueryView`][rerun.catalog.DataframeQueryView] of the recording according to a particular index and content specification.
    ///
    /// The only type of index currently supported is the name of a timeline, or `None` (see below
    /// for details).
    ///
    /// The view will only contain a single row for each unique value of the index
    /// that is associated with a component column that was included in the view.
    /// Component columns that are not included via the view contents will not
    /// impact the rows that make up the view. If the same entity / component pair
    /// was logged to a given index multiple times, only the most recent row will be
    /// included in the view, as determined by the `row_id` column. This will
    /// generally be the last value logged, as row_ids are guaranteed to be
    /// monotonically increasing when data is sent from a single process.
    ///
    /// If `None` is passed as the index, the view will contain only static columns (among those
    /// specified) and no index columns. It will also contain a single row per partition.
    ///
    /// Parameters
    /// ----------
    /// index : str | None
    ///     The index to use for the view. This is typically a timeline name. Use `None` to query static data only.
    /// contents : ViewContentsLike
    ///     The content specification for the view.
    ///
    ///     This can be a single string content-expression such as: `"world/cameras/**"`, or a dictionary
    ///     specifying multiple content-expressions and a respective list of components to select within
    ///     that expression such as `{"world/cameras/**": ["ImageBuffer", "PinholeProjection"]}`.
    /// include_semantically_empty_columns : bool, optional
    ///     Whether to include columns that are semantically empty, by default `False`.
    ///
    ///     Semantically empty columns are components that are `null` or empty `[]` for every row in the recording.
    /// include_tombstone_columns : bool, optional
    ///     Whether to include tombstone columns, by default `False`.
    ///
    ///     Tombstone columns are components used to represent clears. However, even without the clear
    ///     tombstone columns, the view will still apply the clear semantics when resolving row contents.
    ///
    /// Returns
    /// -------
    /// DataframeQueryView
    ///     The view of the dataset.
    #[pyo3(signature = (
        *,
        index,
        contents,
        include_semantically_empty_columns = false,
        include_tombstone_columns = false,
    ))]
    fn dataframe_query_view(
        self_: Py<Self>,
        index: Option<String>,
        contents: Py<PyAny>,
        include_semantically_empty_columns: bool,
        include_tombstone_columns: bool,
        py: Python<'_>,
    ) -> PyResult<PyDataframeQueryView> {
        PyDataframeQueryView::new(
            self_,
            index,
            contents,
            include_semantically_empty_columns,
            include_tombstone_columns,
            py,
        )
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
    fn create_fts_index(
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
    fn create_vector_index(
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
    fn list_indexes(self_: PyRef<'_, Self>) -> PyResult<Vec<PyIndexingResult>> {
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
    fn delete_indexes(
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
    ) -> PyResult<PyDataFusionTable> {
        let connection = self_.client.borrow(self_.py()).connection().clone();
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

        let provider = wait_for_future(self_.py(), async move {
            SearchResultsTableProvider::new(connection.client().await?, dataset_id, request)
                .map_err(to_py_err)?
                .into_provider()
                .await
                .map_err(to_py_err)
        })?;

        let uuid = uuid::Uuid::new_v4().simple();
        let name = format!("{}_search_fts_{uuid}", self_.entry_details.name);

        Ok(PyDataFusionTable {
            client: self_.client.clone_ref(self_.py()),
            name,
            provider,
        })
    }

    /// Search the dataset using a vector search query.
    #[instrument(skip(self_, query, column), err)]
    fn search_vector(
        self_: PyRef<'_, Self>,
        query: VectorLike<'_>,
        column: AnyComponentColumn,
        top_k: u32,
    ) -> PyResult<PyDataFusionTable> {
        let connection = self_.client.borrow(self_.py()).connection().clone();
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

        let provider = wait_for_future(self_.py(), async move {
            SearchResultsTableProvider::new(connection.client().await?, dataset_id, request)
                .map_err(to_py_err)?
                .into_provider()
                .await
                .map_err(to_py_err)
        })?;

        let uuid = uuid::Uuid::new_v4().simple();
        let name = format!("{}_search_vector_{uuid}", self_.entry_details.name);

        Ok(PyDataFusionTable {
            client: self_.client.clone_ref(self_.py()),
            name,
            provider,
        })
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

    pub fn fetch_schema(self_: &PyRef<'_, Self>) -> PyResult<PySchema> {
        let arrow_schema = Self::fetch_arrow_schema(self_)?;
        let schema = SorbetColumnDescriptors::try_from_arrow_fields(None, arrow_schema.fields())
            .map_err(to_py_err)?;

        Ok(PySchema { schema })
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
