use pyo3::exceptions::PyRuntimeError;
use pyo3::{pyclass, pymethods, PyRef, PyResult};
use tokio_stream::StreamExt as _;

use re_chunk::Chunk;
use re_chunk_store::{ChunkStore, ChunkStoreHandle};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::{StoreId, StoreInfo, StoreKind, StoreSource};
use re_protos::common::v1alpha1::ext::DatasetHandle;
use re_protos::frontend::v1alpha1::FetchPartitionRequest;

use crate::catalog::{to_py_err, PyEntry};
use crate::dataframe::PyRecording;
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

    fn download_partition(self_: PyRef<'_, Self>, partition_id: String) -> PyResult<PyRecording> {
        let super_ = self_.as_super();
        let mut client = super_
            .client
            .borrow(self_.py())
            .connection()
            .client()
            .clone();

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

            let mut chunk_stream = catalog_chunk_stream.map(|resp| {
                resp.and_then(|r| {
                    r.chunk
                        .ok_or_else(|| {
                            tonic::Status::internal("missing chunk in FetchPartitionResponse")
                        })?
                        .decode()
                        .map_err(|err| tonic::Status::internal(err.to_string()))
                })
            });

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

            while let Some(result) = chunk_stream.next().await {
                let batch = result.map_err(to_py_err)?;
                let chunk = Chunk::from_record_batch(&batch).map_err(to_py_err)?;

                store
                    .insert_chunk(&std::sync::Arc::new(chunk))
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
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
}
