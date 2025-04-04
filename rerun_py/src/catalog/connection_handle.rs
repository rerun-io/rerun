use arrow::datatypes::Schema as ArrowSchema;
use pyo3::{
    create_exception, exceptions::PyConnectionError, exceptions::PyRuntimeError, PyErr, PyResult,
    Python,
};
use re_chunk::{LatestAtQuery, RangeQuery};
use re_dataframe::ViewContentsSelector;
use tokio_stream::StreamExt as _;

use re_chunk_store::ChunkStore;
use re_grpc_client::redap::{client, get_chunks_response_to_chunk};
use re_log_types::{EntryId, StoreInfo};
use re_protos::common::v1alpha1::IfDuplicateBehavior;
use re_protos::frontend::v1alpha1::frontend_service_client::FrontendServiceClient;
use re_protos::frontend::v1alpha1::{GetDatasetSchemaRequest, RegisterWithDatasetRequest};
use re_protos::manifest_registry::v1alpha1::ext::{DataSource, Query, QueryLatestAt, QueryRange};
use re_protos::{
    catalog::v1alpha1::{
        ext::{DatasetEntry, EntryDetails, TableEntry},
        CreateDatasetEntryRequest, DeleteEntryRequest, EntryFilter, ReadDatasetEntryRequest,
        ReadTableEntryRequest,
    },
    frontend::v1alpha1::GetChunksRequest,
};

use crate::catalog::to_py_err;
use crate::utils::wait_for_future;

create_exception!(catalog, ConnectionError, PyConnectionError);

/// Connection handle to a catalog service.
#[derive(Clone)]
pub struct ConnectionHandle {
    #[expect(dead_code)]
    origin: re_uri::Origin,

    /// The actual tonic connection.
    client: FrontendServiceClient<tonic::transport::Channel>,
}

impl ConnectionHandle {
    pub fn new(py: Python<'_>, origin: re_uri::Origin) -> PyResult<Self> {
        let client = wait_for_future(py, client(origin.clone())).map_err(to_py_err)?;

        Ok(Self { origin, client })
    }

    pub fn client(&self) -> FrontendServiceClient<tonic::transport::Channel> {
        self.client.clone()
    }
}

// TODO(ab): all these request wrapper should be implemented in a more general client wrapper also
// used in e.g. the redap browser, etc. The present connection handle should just forward them.
impl ConnectionHandle {
    pub fn find_entries(
        &mut self,
        py: Python<'_>,
        filter: EntryFilter,
    ) -> PyResult<Vec<EntryDetails>> {
        let response = wait_for_future(
            py,
            self.client
                .find_entries(re_protos::catalog::v1alpha1::FindEntriesRequest {
                    filter: Some(filter),
                }),
        )
        .map_err(to_py_err)?;

        let entries: Result<Vec<_>, _> = response
            .into_inner()
            .entries
            .into_iter()
            .map(TryInto::try_into)
            .collect();

        Ok(entries?)
    }

    pub fn delete_entry(&mut self, py: Python<'_>, entry_id: EntryId) -> PyResult<()> {
        let _response = wait_for_future(
            py,
            self.client.delete_entry(DeleteEntryRequest {
                id: Some(entry_id.into()),
            }),
        )
        .map_err(to_py_err)?;

        Ok(())
    }

    pub fn create_dataset(&mut self, py: Python<'_>, name: String) -> PyResult<DatasetEntry> {
        let response = wait_for_future(
            py,
            self.client
                .create_dataset_entry(CreateDatasetEntryRequest { name: Some(name) }),
        )
        .map_err(to_py_err)?;

        Ok(response
            .into_inner()
            .dataset
            .ok_or(PyRuntimeError::new_err("No dataset in response"))?
            .try_into()?)
    }

    pub fn read_dataset(&mut self, py: Python<'_>, entry_id: EntryId) -> PyResult<DatasetEntry> {
        let response = wait_for_future(
            py,
            self.client.read_dataset_entry(ReadDatasetEntryRequest {
                id: Some(entry_id.into()),
            }),
        )
        .map_err(to_py_err)?;

        Ok(response
            .into_inner()
            .dataset
            .ok_or(PyRuntimeError::new_err("No dataset in response"))?
            .try_into()?)
    }

    pub fn read_table(&mut self, py: Python<'_>, entry_id: EntryId) -> PyResult<TableEntry> {
        let response = wait_for_future(
            py,
            self.client.read_table_entry(ReadTableEntryRequest {
                id: Some(entry_id.into()),
            }),
        )
        .map_err(to_py_err)?;

        Ok(response
            .into_inner()
            .table
            .ok_or(PyRuntimeError::new_err("No table in response"))?
            .try_into()?)
    }

    pub fn get_dataset_schema(
        &mut self,
        py: Python<'_>,
        entry_id: EntryId,
    ) -> PyResult<ArrowSchema> {
        wait_for_future(py, async {
            self.client
                .get_dataset_schema(GetDatasetSchemaRequest {
                    dataset_id: Some(entry_id.into()),
                })
                .await
                .map_err(to_py_err)?
                .into_inner()
                .schema()
                .map_err(to_py_err)
        })
    }

    pub fn register_with_dataset(
        &mut self,
        py: Python<'_>,
        dataset_id: EntryId,
        recording_uri: String,
    ) -> PyResult<()> {
        wait_for_future(py, async {
            self.client
                .register_with_dataset(RegisterWithDatasetRequest {
                    dataset_id: Some(dataset_id.into()),
                    data_sources: vec![DataSource::new_rrd(recording_uri)
                        .map_err(to_py_err)?
                        .into()],
                    //TODO(ab): expose this to as a method argument
                    on_duplicate: IfDuplicateBehavior::Error as i32,
                })
                .await
                .map_err(to_py_err)?;

            Ok(())
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_chunks(
        &mut self,
        py: Python<'_>,
        store_info: StoreInfo,
        dataset_id: EntryId,
        contents: &Option<ViewContentsSelector>,
        latest_at: Option<LatestAtQuery>,
        range: Option<RangeQuery>,
        partition_ids: &[impl AsRef<str> + Sync],
    ) -> PyResult<ChunkStore> {
        let entity_paths = contents
            .as_ref()
            .map_or(vec![], |contents| contents.keys().collect::<Vec<_>>());

        let mut store = ChunkStore::new(store_info.store_id.clone(), Default::default());
        store.set_info(store_info);

        let query = Query {
            latest_at: latest_at.map(|latest_at| QueryLatestAt {
                index: latest_at.timeline().to_string(),
                at: latest_at.at().as_i64(),
                fuzzy_descriptors: vec![], // TODO(jleibs): support this
            }),
            range: range.map(|range| {
                QueryRange {
                    index: range.timeline().to_string(),
                    index_range: range.range,
                    fuzzy_descriptors: vec![], // TODO(jleibs): support this
                }
            }),
            columns_always_include_everything: false,
            columns_always_include_chunk_ids: false,
            columns_always_include_entity_paths: false,
            columns_always_include_byte_offsets: false,
            columns_always_include_static_indexes: false,
            columns_always_include_global_indexes: false,
            columns_always_include_component_indexes: false,
        };

        wait_for_future(py, async {
            let get_chunks_response_stream = self
                .client
                .get_chunks(GetChunksRequest {
                    dataset_id: Some(dataset_id.into()),
                    partition_ids: partition_ids
                        .iter()
                        .map(|id| id.as_ref().to_owned().into())
                        .collect(),
                    chunk_ids: vec![],
                    entity_paths: entity_paths
                        .into_iter()
                        .map(|p| (*p).clone().into())
                        .collect(),
                    query: Some(query.into()),
                })
                .await
                .map_err(to_py_err)?
                .into_inner();

            let mut chunk_stream = get_chunks_response_to_chunk(get_chunks_response_stream);

            while let Some(chunk) = chunk_stream.next().await {
                let chunk = chunk.map_err(to_py_err)?;
                store
                    .insert_chunk(&std::sync::Arc::new(chunk))
                    .map_err(to_py_err)?;
            }

            Ok::<(), PyErr>(())
        })?;

        Ok(store)
    }
}
