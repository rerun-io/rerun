use std::sync::Arc;

use arrow::array::{RecordBatch, StringArray};
use arrow::datatypes::{Field, Schema as ArrowSchema};
use arrow::pyarrow::PyArrowType;
use pyo3::{
    Py, PyAny, PyRef, PyRefMut, PyResult, Python, exceptions::PyRuntimeError, pyclass, pymethods,
};
use tokio_stream::StreamExt as _;

use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_datafusion::{PartitionTableProvider, SearchResultsTableProvider};
use re_grpc_client::get_chunks_response_to_chunk_and_partition_id;
use re_log_encoding::codec::wire::encoder::Encode as _;
use re_log_types::{StoreId, StoreInfo, StoreKind, StoreSource};
use re_protos::catalog::v1alpha1::ext::DatasetDetails;
use re_protos::common::v1alpha1::IfDuplicateBehavior;
use re_protos::common::v1alpha1::ext::DatasetHandle;
use re_protos::frontend::v1alpha1::{CreateIndexRequest, GetChunksRequest, SearchDatasetRequest};
use re_protos::manifest_registry::v1alpha1::ext::IndexProperties;
use re_protos::manifest_registry::v1alpha1::{
    IndexConfig, IndexQueryProperties, InvertedIndexQuery, VectorIndexQuery, index_query_properties,
};
use re_sorbet::{SorbetColumnDescriptors, TimeColumnSelector};

use crate::catalog::task::PyTasks;
use crate::catalog::{
    PyEntry, PyEntryId, VectorDistanceMetricLike, VectorLike,
    dataframe_query::PyDataframeQueryView, to_py_err,
};
use crate::dataframe::{
    AnyComponentColumn, PyDataFusionTable, PyIndexColumnSelector, PyRecording, PySchema,
};
use crate::utils::wait_for_future;

/// A dataset entry in the catalog.
#[pyclass(name = "Dataset", extends=PyEntry)]
pub struct PyDataset {
    pub dataset_details: DatasetDetails,
    pub dataset_handle: DatasetHandle,
}

#[pymethods]
impl PyDataset {
    /// Return the dataset manifest URL.
    //TODO(ab): not sure we want this to be public
    #[getter]
    fn manifest_url(&self) -> String {
        self.dataset_handle.url.to_string()
    }

    /// Return the Arrow schema of the data contained in the dataset.
    //TODO(#9457): there should be another `schema` method which returns a `PySchema`
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
    /// This fails if the change cannot be made to the remote server.
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
    fn partition_url(self_: PyRef<'_, Self>, partition_id: String) -> String {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();

        re_uri::DatasetDataUri {
            origin: connection.origin().clone(),
            dataset_id: super_.details.id.id,
            partition_id,

            //TODO(ab): add support for these two
            time_range: None,
            fragment: Default::default(),
        }
        .to_string()
    }

    /// Register a RRD URI to the dataset and wait for completion.
    ///
    /// This method registers a single recording to the dataset and blocks until the registration is
    /// complete, or after a timeout (in which case, a `TimeoutError` is raised).
    ///
    /// Parameters
    /// ----------
    /// recording_uri: str
    ///     The URI of the RRD to register
    ///
    /// timeout_secs: int
    ///     The timeout after which this method returns.
    #[pyo3(signature = (recording_uri, timeout_secs = 60))]
    fn register(self_: PyRef<'_, Self>, recording_uri: String, timeout_secs: u64) -> PyResult<()> {
        let register_timeout = std::time::Duration::from_secs(timeout_secs);
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let task_ids =
            connection.register_with_dataset(self_.py(), dataset_id, vec![recording_uri])?;

        connection.wait_for_tasks(self_.py(), &task_ids, register_timeout)
    }

    /// Register a batch of RRD URIs to the dataset and return a handle to the tasks.
    ///
    /// This method initiates the registration of multiple recordings to the dataset, and returns
    /// the corresponding task ids in a [`Tasks`] object.
    ///
    /// Parameters
    /// ----------
    /// recording_uris: list[str]
    ///     The URIs of the RRDs to register
    #[allow(rustdoc::broken_intra_doc_links)]
    fn register_batch(self_: PyRef<'_, Self>, recording_uris: Vec<String>) -> PyResult<PyTasks> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let task_ids = connection.register_with_dataset(self_.py(), dataset_id, recording_uris)?;

        Ok(PyTasks::new(super_.client.clone_ref(self_.py()), task_ids))
    }

    /// Download a partition from the dataset.
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
                    query: None,
                })
                .await
                .map_err(to_py_err)?
                .into_inner();

            let store_id = StoreId::from_string(StoreKind::Recording, partition_id);
            let store_info = StoreInfo {
                application_id: dataset_name.into(),
                store_id: store_id.clone(),
                cloned_from: None,
                store_source: StoreSource::Unknown,
                store_version: None,
            };

            let mut store = ChunkStore::new(store_id, Default::default());
            store.set_info(store_info);

            let mut chunk_stream =
                get_chunks_response_to_chunk_and_partition_id(catalog_chunk_stream);

            while let Some(chunk) = chunk_stream.next().await {
                let (chunk, _partition_id) = chunk.map_err(to_py_err)?;
                store
                    .insert_chunk(&std::sync::Arc::new(chunk))
                    .map_err(to_py_err)?;
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

    /// Create a view to run a dataframe query on the dataset.
    #[expect(clippy::fn_params_excessive_bools)]
    #[pyo3(signature = (
        *,
        index,
        contents,
        include_semantically_empty_columns = false,
        include_indicator_columns = false,
        include_tombstone_columns = false,
    ))]
    fn dataframe_query_view(
        self_: Py<Self>,
        index: String,
        contents: Py<PyAny>,
        include_semantically_empty_columns: bool,
        include_indicator_columns: bool,
        include_tombstone_columns: bool,
        py: Python<'_>,
    ) -> PyResult<PyDataframeQueryView> {
        PyDataframeQueryView::new(
            self_,
            index,
            contents,
            include_semantically_empty_columns,
            include_indicator_columns,
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
            dataset_id: Some(dataset_id.into()),

            partition_ids: vec![],

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
                .create_index(request)
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

        let distance_metric: re_protos::manifest_registry::v1alpha1::VectorDistanceMetric =
            distance_metric.try_into()?;

        let properties = IndexProperties::VectorIvfPq {
            num_partitions,
            num_sub_vectors,
            metric: distance_metric,
        };

        let request = CreateIndexRequest {
            dataset_id: Some(dataset_id.into()),

            partition_ids: vec![],

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
                .create_index(request)
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            Ok(())
        })
    }

    /// Search the dataset using a full-text search query.
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

        let query = RecordBatch::try_new(
            Arc::new(schema),
            vec![Arc::new(StringArray::from_iter_values([query]))],
        )
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        let request = SearchDatasetRequest {
            dataset_id: Some(dataset_id.into()),
            column: Some(component_descriptor.0.into()),
            properties: Some(IndexQueryProperties {
                props: Some(
                    re_protos::manifest_registry::v1alpha1::index_query_properties::Props::Inverted(
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
            SearchResultsTableProvider::new(connection.client().await?, request)
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
            dataset_id: Some(dataset_id.into()),
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
            SearchResultsTableProvider::new(connection.client().await?, request)
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
}

impl PyDataset {
    fn fetch_arrow_schema(self_: &PyRef<'_, Self>) -> PyResult<ArrowSchema> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow_mut(self_.py()).connection().clone();

        let schema = connection.get_dataset_schema(self_.py(), super_.details.id)?;

        Ok(schema)
    }

    fn fetch_schema(self_: &PyRef<'_, Self>) -> PyResult<PySchema> {
        Self::fetch_arrow_schema(self_).and_then(|arrow_schema| {
            let schema =
                SorbetColumnDescriptors::try_from_arrow_fields(None, arrow_schema.fields())
                    .map_err(to_py_err)?;

            Ok(PySchema { schema })
        })
    }
}
