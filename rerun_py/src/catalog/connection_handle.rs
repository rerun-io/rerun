use arrow::array::RecordBatch;
use arrow::datatypes::Schema as ArrowSchema;
use pyo3::exceptions::PyValueError;
use pyo3::{
    PyErr, PyResult, Python, create_exception, exceptions::PyConnectionError,
    exceptions::PyRuntimeError,
};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::manifest_registry::v1alpha1::RegisterWithDatasetResponse;
use std::collections::BTreeMap;
use tokio_stream::StreamExt as _;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{LatestAtQuery, RangeQuery};
use re_chunk_store::ChunkStore;
use re_dataframe::ViewContentsSelector;
use re_grpc_client::redap::{RedapClient, get_chunks_response_to_chunk_and_partition_id};
use re_log_types::{ApplicationId, EntryId, StoreId, StoreInfo, StoreKind, StoreSource};
use re_protos::{
    catalog::v1alpha1::{
        CreateDatasetEntryRequest, DeleteEntryRequest, EntryFilter, ReadDatasetEntryRequest,
        ReadTableEntryRequest,
        ext::{DatasetEntry, EntryDetails, TableEntry},
    },
    common::v1alpha1::{IfDuplicateBehavior, TaskId},
    frontend::v1alpha1::{GetChunksRequest, GetDatasetSchemaRequest, RegisterWithDatasetRequest},
    manifest_registry::v1alpha1::ext::{DataSource, Query, QueryLatestAt, QueryRange},
};

use crate::catalog::to_py_err;
use crate::utils::wait_for_future;

create_exception!(catalog, ConnectionError, PyConnectionError);

/// Connection handle to a catalog service.
#[derive(Clone)]
pub struct ConnectionHandle {
    origin: re_uri::Origin,

    /// The actual tonic connection.
    client: RedapClient,
}

impl ConnectionHandle {
    pub fn new(py: Python<'_>, origin: re_uri::Origin) -> PyResult<Self> {
        let client = wait_for_future(py, re_grpc_client::redap::client(origin.clone()))
            .map_err(to_py_err)?;

        Ok(Self { origin, client })
    }

    pub fn client(&self) -> RedapClient {
        self.client.clone()
    }

    pub fn origin(&self) -> &re_uri::Origin {
        &self.origin
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

    /// Initiate registration of the provided recording URIs with a dataset and return the
    /// corresponding task IDs.
    ///
    /// NOTE: The server may pool multiple registrations into a single task. The result always has
    /// the same length as the output, so task ids may be duplicated.
    pub fn register_with_dataset(
        &mut self,
        py: Python<'_>,
        dataset_id: EntryId,
        recording_uris: Vec<String>,
    ) -> PyResult<Vec<TaskId>> {
        wait_for_future(py, async {
            let data_sources = recording_uris
                .iter()
                .map(|uri| DataSource::new_rrd(uri).map(Into::into))
                .collect::<Result<Vec<_>, _>>()
                .map_err(to_py_err)?;

            let response = self
                .client
                .register_with_dataset(RegisterWithDatasetRequest {
                    dataset_id: Some(dataset_id.into()),
                    data_sources,
                    //TODO(ab): expose this to as a method argument
                    on_duplicate: IfDuplicateBehavior::Error as i32,
                })
                .await
                .map_err(to_py_err)?
                .into_inner()
                .data
                .ok_or_else(|| PyValueError::new_err("missing data from response"))?
                .decode()
                .map_err(to_py_err)?;

            // TODO(andrea): why is the schema completely off?
            #[expect(clippy::overly_complex_bool_expr)]
            if false
                && !response
                    .schema()
                    .contains(&RegisterWithDatasetResponse::schema())
            {
                return Err(PyValueError::new_err(
                    "invalid schema for RegisterWithDatasetResponse",
                ));
            }

            response
                .column_by_name(RegisterWithDatasetResponse::TASK_ID)
                .and_then(|column| {
                    column
                        .try_downcast_array_ref::<arrow::array::StringArray>()
                        .ok()
                        .map(|col| {
                            col.iter()
                                .filter_map(|v| v.map(|id| TaskId { id: id.to_owned() }))
                                .collect::<Vec<_>>()
                        })
                })
                .ok_or_else(|| PyValueError::new_err("bug: invalid response schema"))
        })
    }

    pub fn query_tasks(&mut self, py: Python<'_>, task_ids: &[TaskId]) -> PyResult<RecordBatch> {
        wait_for_future(py, async {
            let request = re_protos::redap_tasks::v1alpha1::QueryTasksRequest {
                ids: task_ids.to_vec(),
            };

            let status_table = self
                .client
                .query_tasks(request)
                .await
                .map_err(to_py_err)?
                .into_inner()
                .dataframe_part()
                .map_err(to_py_err)?
                .decode()
                .map_err(to_py_err)?;

            Ok(status_table)
        })
    }

    /// Wait for the provided tasks to finish.
    pub fn wait_for_tasks(
        &mut self,
        py: Python<'_>,
        task_ids: &[TaskId],
        timeout: std::time::Duration,
    ) -> PyResult<()> {
        wait_for_future(py, async {
            let timeout: prost_types::Duration = timeout.try_into().map_err(|err| {
                PyValueError::new_err(format!(
                    "failed to convert timeout to serialized duration: {err}"
                ))
            })?;
            let request = re_protos::redap_tasks::v1alpha1::QueryTasksOnCompletionRequest {
                ids: task_ids.to_vec(),
                timeout: Some(timeout),
            };
            let mut response_stream = self
                .client
                .query_tasks_on_completion(request)
                .await
                .map_err(to_py_err)?
                .into_inner();

            // cheat: we know there is only one item in the stream
            let tasks_statuses = if let Some(response) = response_stream.next().await {
                response
                    .map_err(to_py_err)?
                    .data
                    .ok_or_else(|| PyValueError::new_err("received response without data"))?
                    .decode()
                    .map_err(to_py_err)?
            } else {
                return Err(PyConnectionError::new_err("no response from task"));
            };

            // TODO(andrea): this is a bit hideous. Maybe the idea of returning a dataframe rather
            // than a nicely typed object should be revisited.

            let exec_statuses = tasks_statuses
                .column_by_name("exec_status")
                .ok_or_else(|| {
                    PyValueError::new_err("bug: exec_status column not found in response")
                })?
                .try_downcast_array_ref::<arrow::array::StringArray>()
                .map_err(to_py_err)?;

            let msgs = tasks_statuses
                .column_by_name("msgs")
                .ok_or_else(|| PyValueError::new_err("bug: msgs column not found in response"))?
                .try_downcast_array_ref::<arrow::array::StringArray>()
                .map_err(to_py_err)?;

            // report the first non success error
            for (i, status) in exec_statuses.iter().enumerate() {
                let status = status.ok_or_else(|| PyValueError::new_err("missing status value"))?;

                if status != "success" {
                    return Err(PyValueError::new_err(format!(
                        "task failed: {}",
                        msgs.value(i)
                    )));
                }
            }

            Ok(())
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_chunks_for_dataframe_query(
        &mut self,
        py: Python<'_>,
        dataset_id: EntryId,
        contents: &Option<ViewContentsSelector>,
        latest_at: Option<LatestAtQuery>,
        range: Option<RangeQuery>,
        partition_ids: &[impl AsRef<str> + Sync],
    ) -> PyResult<BTreeMap<String, ChunkStore>> {
        let entity_paths = contents
            .as_ref()
            .map_or(vec![], |contents| contents.keys().collect::<Vec<_>>());

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

        let mut stores = BTreeMap::default();

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

            let mut chunk_stream =
                get_chunks_response_to_chunk_and_partition_id(get_chunks_response_stream);

            while let Some(chunk_and_partition_id) = chunk_stream.next().await {
                let (chunk, partition_id) = chunk_and_partition_id.map_err(to_py_err)?;

                let partition_id = partition_id.ok_or_else(|| {
                    PyValueError::new_err("Received chunk without a partition id")
                })?;

                let store = stores.entry(partition_id.clone()).or_insert_with(|| {
                    let store_info = StoreInfo {
                        application_id: ApplicationId::from(partition_id),
                        store_id: StoreId::random(StoreKind::Recording),
                        cloned_from: None,
                        store_source: StoreSource::Unknown,
                        store_version: None,
                    };

                    let mut store =
                        ChunkStore::new(store_info.store_id.clone(), Default::default());
                    store.set_info(store_info);
                    store
                });

                store
                    .insert_chunk(&std::sync::Arc::new(chunk))
                    .map_err(to_py_err)?;
            }

            Ok::<_, PyErr>(())
        })?;

        Ok(stores)
    }
}
