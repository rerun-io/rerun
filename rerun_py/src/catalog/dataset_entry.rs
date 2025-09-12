use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchOptions, StringArray};
use arrow::datatypes::{Field, Schema as ArrowSchema};
use arrow::pyarrow::PyArrowType;
use pyo3::Bound;
use pyo3::types::PyAnyMethods as _;
use pyo3::{
    Py, PyAny, PyRef, PyRefMut, PyResult, Python, exceptions::PyRuntimeError,
    exceptions::PyValueError, pyclass, pymethods,
};
use tokio_stream::StreamExt as _;
use tracing::instrument;

use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_datafusion::{PartitionTableProvider, SearchResultsTableProvider};
use re_log_encoding::codec::wire::encoder::Encode as _;
use re_log_types::{StoreId, StoreInfo, StoreKind, StoreSource};
use re_protos::cloud::v1alpha1::ext::DatasetDetails;
use re_protos::cloud::v1alpha1::ext::IndexProperties;
use re_protos::cloud::v1alpha1::{CreateIndexRequest, GetChunksRequest, SearchDatasetRequest};
use re_protos::cloud::v1alpha1::{
    IndexConfig, IndexQueryProperties, InvertedIndexQuery, VectorIndexQuery, index_query_properties,
};
use re_protos::common::v1alpha1::IfDuplicateBehavior;
use re_protos::common::v1alpha1::ext::DatasetHandle;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::get_chunks_response_to_chunk_and_partition_id;
use re_sorbet::{SorbetColumnDescriptors, TimeColumnSelector};

use crate::dataframe::{AnyComponentColumn, PyIndexColumnSelector, PyRecording, PySchema};
use crate::utils::wait_for_future;

use super::{
    PyDataFusionTable, PyEntry, PyEntryId, VectorDistanceMetricLike, VectorLike,
    dataframe_query::PyDataframeQueryView, task::PyTasks, to_py_err,
};

/// A dataset entry in the catalog.
#[pyclass(name = "DatasetEntry", extends=PyEntry)]
pub struct PyDatasetEntry {
    pub dataset_details: DatasetDetails,
    pub dataset_handle: DatasetHandle,
}

#[pymethods]
impl PyDatasetEntry {
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

        let super_ = self_.as_super();
        let client = super_.client.clone_ref(py);
        let connection = super_.client.borrow(py).connection().clone();

        let dataset_entry = connection.read_dataset(py, blueprint_dataset_entry_id)?;

        let entry = PyEntry {
            client,
            id: Py::new(
                py,
                PyEntryId {
                    id: blueprint_dataset_entry_id,
                },
            )?,
            details: dataset_entry.details,
        };

        let dataset = Self {
            dataset_details: dataset_entry.dataset_details,
            dataset_handle: dataset_entry.handle,
        };

        Some(Py::new(py, (dataset, entry))).transpose()
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
        let super_ = self_.as_super();
        let connection = super_.client.borrow(py).connection().clone();
        let dataset_id = super_.details.id;

        let mut dataset_details = self_.dataset_details.clone();
        dataset_details.default_blueprint = partition_id.map(Into::into);

        let result = connection.update_dataset(py, dataset_id, dataset_details)?;

        self_.dataset_details = result.dataset_details;

        Ok(())
    }

    /// Return the schema of the data contained in the dataset.
    fn schema(self_: PyRef<'_, Self>) -> PyResult<PySchema> {
        Self::fetch_schema(&self_)
    }

    /// Returns a list of partitions IDs for the dataset.
    fn partition_ids(self_: PyRef<'_, Self>) -> PyResult<Vec<String>> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        connection.get_dataset_partition_ids(self_.py(), dataset_id)
    }

    /// Return the partition table as a Datafusion table provider.
    #[instrument(skip_all)]
    fn partition_table(self_: PyRef<'_, Self>) -> PyResult<PyDataFusionTable> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let provider = wait_for_future(self_.py(), async move {
            PartitionTableProvider::new(connection.client().await?, dataset_id)
                .into_provider()
                .await
                .map_err(to_py_err)
        })?;

        #[expect(clippy::string_add)]
        Ok(PyDataFusionTable {
            client: super_.client.clone_ref(self_.py()),
            name: super_.name() + "_partition_table",
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
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();

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
            dataset_id: super_.details.id.id,
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
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let mut results = connection.register_with_dataset(
            self_.py(),
            dataset_id,
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
    #[allow(rustdoc::broken_intra_doc_links)]
    #[pyo3(signature = (
        recording_uris,
        *,
        recording_layers = vec![],
    ))]
    #[pyo3(text_signature = "(self, /, recording_uris, *, recording_layers = [])")]
    // TODO(ab): it might be useful to return partition ids directly since we have them
    fn register_batch(
        self_: PyRef<'_, Self>,
        recording_uris: Vec<String>,
        recording_layers: Vec<String>,
    ) -> PyResult<PyTasks> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let results = connection.register_with_dataset(
            self_.py(),
            dataset_id,
            recording_uris,
            recording_layers,
        )?;

        Ok(PyTasks::new(
            super_.client.clone_ref(self_.py()),
            results.into_iter().map(|desc| desc.task_id),
        ))
    }

    /// Download a partition from the dataset.
    #[instrument(skip(self_), err)]
    fn download_partition(self_: PyRef<'_, Self>, partition_id: String) -> PyResult<PyRecording> {
        let super_ = self_.as_super();
        let catalog_client = super_.client.borrow(self_.py());
        let connection = catalog_client.connection();

        let dataset_id = super_.details.id;
        let dataset_name = super_.details.name.clone();

        //TODO(ab): use `ConnectionHandle::get_chunk()`
        let store: PyResult<ChunkStore> = wait_for_future(self_.py(), async move {
            let catalog_chunk_stream = connection
                .client()
                .await?
                .inner()
                .get_chunks(GetChunksRequest {
                    dataset_id: Some(dataset_id.into()),
                    partition_ids: vec![partition_id.clone().into()],
                    chunk_ids: vec![],
                    entity_paths: vec![],
                    select_all_entity_paths: true,
                    fuzzy_descriptors: vec![],
                    exclude_static_data: false,
                    exclude_temporal_data: false,
                    query: None,
                })
                .await
                .map_err(to_py_err)?
                .into_inner();

            let store_id = StoreId::new(StoreKind::Recording, dataset_name, partition_id.clone());
            let store_info = StoreInfo {
                store_id: store_id.clone(),
                cloned_from: None,
                store_source: StoreSource::Unknown,
                store_version: None,
            };

            let mut store = ChunkStore::new(store_id, Default::default());
            store.set_store_info(store_info);

            let mut chunk_stream =
                get_chunks_response_to_chunk_and_partition_id(catalog_chunk_stream);

            while let Some(chunks) = chunk_stream.next().await {
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

    #[allow(rustdoc::private_doc_tests, rustdoc::invalid_rust_codeblocks)]
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
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;
        let time_selector: TimeColumnSelector = time_index.into();

        let schema = Self::fetch_schema(&self_)?;
        let component_descriptor = schema.column_for_selector(column)?;

        let properties = IndexProperties::Inverted {
            store_position,
            base_tokenizer: base_tokenizer.into(),
        };

        let request = CreateIndexRequest {
            partition_ids: vec![],
            partition_layers: vec![],

            config: Some(IndexConfig {
                properties: Some(properties.into()),
                column: Some(component_descriptor.0.into()),
                time_index: Some(time_selector.timeline.into()),
            }),

            on_duplicate: IfDuplicateBehavior::Overwrite as i32,
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
    #[pyo3(signature = (
        *,
        column,
        time_index,
        num_partitions = 5,
        num_sub_vectors = 16,
        distance_metric = VectorDistanceMetricLike::VectorDistanceMetric(crate::catalog::PyVectorDistanceMetric::Cosine),
    ))]
    #[instrument(skip(self_, column, time_index, distance_metric), err)]
    fn create_vector_index(
        self_: PyRef<'_, Self>,
        column: AnyComponentColumn,
        time_index: PyIndexColumnSelector,
        num_partitions: usize,
        num_sub_vectors: usize,
        distance_metric: VectorDistanceMetricLike,
    ) -> PyResult<()> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let time_selector: TimeColumnSelector = time_index.into();

        let schema = Self::fetch_schema(&self_)?;
        let component_descriptor = schema.column_for_selector(column)?;

        let distance_metric: re_protos::cloud::v1alpha1::VectorDistanceMetric =
            distance_metric.try_into()?;

        let properties = IndexProperties::VectorIvfPq {
            num_partitions,
            num_sub_vectors,
            metric: distance_metric,
        };

        let request = CreateIndexRequest {
            partition_ids: vec![],
            partition_layers: vec![],

            config: Some(IndexConfig {
                properties: Some(properties.into()),
                column: Some(component_descriptor.0.into()),
                time_index: Some(time_selector.timeline.into()),
            }),

            on_duplicate: IfDuplicateBehavior::Overwrite as i32,
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

    /// Search the dataset using a full-text search query.
    #[instrument(skip(self_, column), err)]
    fn search_fts(
        self_: PyRef<'_, Self>,
        query: String,
        column: AnyComponentColumn,
    ) -> PyResult<PyDataFusionTable> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

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
            query: Some(
                query
                    .encode()
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
            ),
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
        let name = format!("{}_search_fts_{uuid}", super_.name());

        Ok(PyDataFusionTable {
            client: super_.client.clone_ref(self_.py()),
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
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

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
            query: Some(
                query
                    .encode()
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
            ),
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
        let name = format!("{}_search_vector_{uuid}", super_.name());

        Ok(PyDataFusionTable {
            client: super_.client.clone_ref(self_.py()),
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
    #[allow(clippy::fn_params_excessive_bools)]
    fn do_maintenance(
        self_: PyRef<'_, Self>,
        py: Python<'_>,
        optimize_indexes: bool,
        retrain_indexes: bool,
        compact_fragments: bool,
        cleanup_before: Option<Bound<'_, PyAny>>,
        unsafe_allow_recent_cleanup: bool,
    ) -> PyResult<()> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

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
            dataset_id,
            optimize_indexes,
            retrain_indexes,
            compact_fragments,
            cleanup_before,
            unsafe_allow_recent_cleanup,
        )
    }
}

impl PyDatasetEntry {
    pub fn fetch_arrow_schema(self_: &PyRef<'_, Self>) -> PyResult<ArrowSchema> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow_mut(self_.py()).connection().clone();

        let schema = connection.get_dataset_schema(self_.py(), super_.details.id)?;

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
