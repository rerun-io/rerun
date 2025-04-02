use std::sync::Arc;

use arrow::array::{Float32Array, RecordBatch, StringArray};
use arrow::datatypes::{Field, Schema as ArrowSchema};
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::PyRuntimeError;
use pyo3::{pyclass, pymethods, FromPyObject, PyRef, PyResult};
use re_dataframe::ComponentColumnSelector;
use re_datafusion::SearchResultsTableProvider;
use re_log_encoding::codec::wire::encoder::Encode as _;
use re_protos::manifest_registry::v1alpha1::{
    index_query_properties, IndexColumn, IndexQueryProperties, InvertedIndexQuery, VectorIndexQuery,
};
use re_sdk::ComponentDescriptor;
use tokio_stream::StreamExt as _;

use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_grpc_client::redap::fetch_partition_response_to_chunk;
use re_log_types::{StoreId, StoreInfo, StoreKind, StoreSource};
use re_protos::common::v1alpha1::ext::DatasetHandle;
use re_protos::frontend::v1alpha1::{FetchPartitionRequest, SearchDatasetRequest};

use crate::catalog::{to_py_err, PyEntry};
use crate::dataframe::{PyComponentColumnSelector, PyDataFusionTable, PyRecording};
use crate::utils::wait_for_future;

#[pyclass(name = "Dataset", extends=PyEntry)]
pub struct PyDataset {
    pub dataset_handle: DatasetHandle,
}

#[pymethods]
impl PyDataset {
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

    /// Register a RRD URI to the dataset.
    fn register(self_: PyRef<'_, Self>, recording_uri: String) -> PyResult<()> {
        let super_ = self_.as_super();
        let mut connection = super_.client.borrow(self_.py()).connection().clone();
        let dataset_id = super_.details.id;

        connection.register_with_dataset(self_.py(), dataset_id, recording_uri)
    }

    fn download_partition(self_: PyRef<'_, Self>, partition_id: String) -> PyResult<PyRecording> {
        let super_ = self_.as_super();
        let mut client = super_.client.borrow(self_.py()).connection().client();

        let dataset_id = super_.details.id;
        let dataset_name = super_.details.name.clone();

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

        Ok(PyDataFusionTable { provider })
    }

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

        Ok(PyDataFusionTable { provider })
    }
}

/// A type alias for a vector (vector search input data).
#[derive(FromPyObject)]
enum VectorLike<'py> {
    NumPy(numpy::PyArrayLike1<'py, f32>),
    Vector(Vec<f32>),
}

impl VectorLike<'_> {
    fn to_record_batch(&self) -> PyResult<RecordBatch> {
        let schema = arrow::datatypes::Schema::new_with_metadata(
            vec![Field::new(
                "items",
                arrow::datatypes::DataType::Float32,
                false,
            )],
            Default::default(),
        );

        match self {
            VectorLike::NumPy(array) => {
                let floats: Vec<f32> = array
                    .as_array()
                    .as_slice()
                    .ok_or_else(|| {
                        PyRuntimeError::new_err("Failed to convert numpy array to slice".to_owned())
                    })?
                    .to_vec();

                RecordBatch::try_new(Arc::new(schema), vec![Arc::new(Float32Array::from(floats))])
                    .map_err(|err| {
                        PyRuntimeError::new_err(format!("Failed to create RecordBatches: {err}"))
                    })
            }
            VectorLike::Vector(floats) => RecordBatch::try_new(
                Arc::new(schema),
                vec![Arc::new(Float32Array::from(floats.clone()))],
            )
            .map_err(|err| {
                PyRuntimeError::new_err(format!("Failed to create RecordBatches: {err}"))
            }),
        }
    }
}
