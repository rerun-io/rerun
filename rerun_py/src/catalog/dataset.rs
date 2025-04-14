use std::sync::Arc;

use arrow::array::{RecordBatch, StringArray};
use arrow::datatypes::{Field, Schema as ArrowSchema};
use arrow::pyarrow::PyArrowType;
use pyo3::{exceptions::PyRuntimeError, pyclass, pymethods, Py, PyAny, PyRef, PyResult, Python};
use tokio_stream::StreamExt as _;

use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_dataframe::{ComponentColumnSelector, TimeColumnSelector};
use re_datafusion::{PartitionTableProvider, SearchResultsTableProvider};
use re_grpc_client::redap::fetch_partition_response_to_chunk;
use re_log_encoding::codec::wire::encoder::Encode as _;
use re_log_types::{StoreId, StoreInfo, StoreKind, StoreSource};
use re_protos::common::v1alpha1::ext::DatasetHandle;
use re_protos::common::v1alpha1::IfDuplicateBehavior;
use re_protos::frontend::v1alpha1::{
    CreateIndexRequest, FetchPartitionRequest, SearchDatasetRequest,
};
use re_protos::manifest_registry::v1alpha1::ext::IndexProperties;
use re_protos::manifest_registry::v1alpha1::{
    index_query_properties, IndexColumn, IndexConfig, IndexQueryProperties, InvertedIndexQuery,
    VectorIndexQuery,
};
use re_sdk::{ComponentDescriptor, ComponentName};

use crate::catalog::{
    dataframe_query::PyDataframeQueryView, to_py_err, PyEntry, VectorDistanceMetricLike, VectorLike,
};
use crate::dataframe::{
    PyComponentColumnSelector, PyDataFusionTable, PyIndexColumnSelector, PyRecording,
};
use crate::utils::wait_for_future;

/// A dataset entry in the catalog.
#[pyclass(name = "Dataset", extends=PyEntry)]
pub struct PyDataset {
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
        let super_ = self_.as_super();
        let mut connection = super_.client.borrow_mut(self_.py()).connection().clone();

        let schema = connection.get_dataset_schema(self_.py(), super_.details.id)?;

        Ok(schema.into())
    }

    /// Return the partition table as a Datafusion table provider.
    fn partition_table(self_: PyRef<'_, Self>) -> PyResult<PyDataFusionTable> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let provider = wait_for_future(self_.py(), async move {
            PartitionTableProvider::new(connection.client(), dataset_id)
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

    /// Register a RRD URI to the dataset.
    fn register(self_: PyRef<'_, Self>, recording_uri: String) -> PyResult<()> {
        // TODO(#9731): In order to make the `register` method to appear synchronous,
        // we need to hard-code a max timeout for waiting for the task.
        // 60 seconds is totally arbitrary but should work for now.
        //
        // A more permanent solution is to expose an asynchronous register method, and/or
        // the timeout directly to the caller.
        // See also issue #9731
        const MAX_REGISTER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
        let super_ = self_.as_super();
        let mut connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let task_id = connection.register_with_dataset(self_.py(), dataset_id, recording_uri)?;
        connection.wait_for_task(self_.py(), &task_id, MAX_REGISTER_TIMEOUT)
    }

    /// Download a partition from the dataset.
    fn download_partition(self_: PyRef<'_, Self>, partition_id: String) -> PyResult<PyRecording> {
        let super_ = self_.as_super();
        let mut client = super_.client.borrow(self_.py()).connection().client();

        let dataset_id = super_.details.id;
        let dataset_name = super_.details.name.clone();

        //TODO(ab): use `ConnectionHandle::get_chunk()`
        let store: PyResult<ChunkStore> = wait_for_future(self_.py(), async move {
            let catalog_chunk_stream = client
                .fetch_partition(FetchPartitionRequest {
                    dataset_id: Some(dataset_id.into()),
                    partition_id: Some(partition_id.clone().into()),
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

            let mut chunk_stream = fetch_partition_response_to_chunk(catalog_chunk_stream);

            while let Some(chunk) = chunk_stream.next().await {
                let chunk = chunk.map_err(to_py_err)?;
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
        column: PyComponentColumnSelector,
        time_index: PyIndexColumnSelector,
        store_position: bool,
        base_tokenizer: &str,
    ) -> PyResult<()> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let time_selector: TimeColumnSelector = time_index.into();
        let column_selector: ComponentColumnSelector = column.into();
        let mut component_descriptor =
            ComponentDescriptor::new(column_selector.component_name.clone());

        // TODO(jleibs): get rid of this hack
        if component_descriptor.component_name == ComponentName::from("rerun.components.Text") {
            component_descriptor = component_descriptor
                .or_with_archetype_name(|| "rerun.archetypes.TextLog".into())
                .or_with_archetype_field_name(|| "text".into());
        }

        let properties = IndexProperties::Inverted {
            store_position,
            base_tokenizer: base_tokenizer.into(),
        };

        let request = CreateIndexRequest {
            dataset_id: Some(dataset_id.into()),

            partition_ids: vec![],

            config: Some(IndexConfig {
                properties: Some(properties.into()),
                column: Some(IndexColumn {
                    entity_path: Some(column_selector.entity_path.into()),
                    component: Some(component_descriptor.into()),
                }),
                time_index: Some(time_selector.timeline.into()),
            }),

            on_duplicate: IfDuplicateBehavior::Overwrite as i32,
        };

        wait_for_future(self_.py(), async {
            connection
                .client()
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
        column: PyComponentColumnSelector,
        time_index: PyIndexColumnSelector,
        num_partitions: usize,
        num_sub_vectors: usize,
        distance_metric: VectorDistanceMetricLike,
    ) -> PyResult<()> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let time_selector: TimeColumnSelector = time_index.into();
        let column_selector: ComponentColumnSelector = column.into();
        let component_descriptor = ComponentDescriptor::new(column_selector.component_name.clone());

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
                column: Some(IndexColumn {
                    entity_path: Some(column_selector.entity_path.into()),
                    component: Some(component_descriptor.into()),
                }),
                time_index: Some(time_selector.timeline.into()),
            }),

            on_duplicate: IfDuplicateBehavior::Overwrite as i32,
        };

        wait_for_future(self_.py(), async {
            connection
                .client()
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
        column: PyComponentColumnSelector,
    ) -> PyResult<PyDataFusionTable> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let column_selector: ComponentColumnSelector = column.into();
        let component_descriptor = ComponentDescriptor::new(column_selector.component_name.clone());

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
            column: Some(IndexColumn {
                entity_path: Some(column_selector.entity_path.into()),
                component: Some(component_descriptor.into()),
            }),
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
            SearchResultsTableProvider::new(connection.client(), request)
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
        column: PyComponentColumnSelector,
        top_k: u32,
    ) -> PyResult<PyDataFusionTable> {
        let super_ = self_.as_super();
        let connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        let column_selector: ComponentColumnSelector = column.into();
        let component_descriptor = ComponentDescriptor::new(column_selector.component_name.clone());

        let query = query.to_record_batch()?;

        let request = SearchDatasetRequest {
            dataset_id: Some(dataset_id.into()),
            column: Some(IndexColumn {
                entity_path: Some(column_selector.entity_path.into()),
                component: Some(component_descriptor.into()),
            }),
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
            SearchResultsTableProvider::new(connection.client(), request)
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
