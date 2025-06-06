use arrow::array::RecordBatch;
use arrow::datatypes::Schema as ArrowSchema;
use pyo3::exceptions::PyValueError;
use pyo3::{
    PyErr, PyResult, Python, create_exception, exceptions::PyConnectionError,
    exceptions::PyRuntimeError,
};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_protos::manifest_registry::v1alpha1::RegisterWithDatasetResponse;
use re_protos::redap_tasks::v1alpha1::QueryTasksResponse;
use std::collections::BTreeMap;
use tokio_stream::StreamExt as _;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk::{LatestAtQuery, RangeQuery};
use re_chunk_store::ChunkStore;
use re_dataframe::ViewContentsSelector;
use re_grpc_client::{
    ConnectionClient, ConnectionRegistryHandle, get_chunks_response_to_chunk_and_partition_id,
};
use re_log_types::{ApplicationId, EntryId, StoreId, StoreInfo, StoreKind, StoreSource};
use re_protos::catalog::v1alpha1::ext::{DatasetDetails, UpdateDatasetEntryRequest};
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

    connection_registry: ConnectionRegistryHandle,
}

impl ConnectionHandle {
    pub fn new(connection_registry: ConnectionRegistryHandle, origin: re_uri::Origin) -> Self {
        Self {
            origin,
            connection_registry,
        }
    }

    pub async fn client(&self) -> PyResult<ConnectionClient> {
        self.connection_registry
            .client(self.origin.clone())
            .await
            .map_err(to_py_err)
    }

    pub fn origin(&self) -> &re_uri::Origin {
        &self.origin
    }
}

// TODO(ab): migrate all of this to some `RedapClient` wrapper to be provided by `ConnectionRegistry`
impl ConnectionHandle {
    pub fn find_entries(&self, py: Python<'_>, filter: EntryFilter) -> PyResult<Vec<EntryDetails>> {
        let response = wait_for_future(py, async {
            (self.client().await?)
                .inner()
                .find_entries(re_protos::catalog::v1alpha1::FindEntriesRequest {
                    filter: Some(filter),
                })
                .await
                .map_err(to_py_err)
        })?;

        let entries: Result<Vec<_>, _> = response
            .into_inner()
            .entries
            .into_iter()
            .map(TryInto::try_into)
            .collect();

        Ok(entries?)
    }

    pub fn delete_entry(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<()> {
        let _response = wait_for_future(py, async {
            (self.client().await?)
                .inner()
                .delete_entry(DeleteEntryRequest {
                    id: Some(entry_id.into()),
                })
                .await
                .map_err(to_py_err)
        })?;

        Ok(())
    }

    pub fn create_dataset(&self, py: Python<'_>, name: String) -> PyResult<DatasetEntry> {
        let response = wait_for_future(py, async {
            (self.client().await?)
                .inner()
                .create_dataset_entry(CreateDatasetEntryRequest { name: Some(name) })
                .await
                .map_err(to_py_err)
        })?;

        Ok(response
            .into_inner()
            .dataset
            .ok_or(PyRuntimeError::new_err("No dataset in response"))?
            .try_into()?)
    }

    pub fn read_dataset(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<DatasetEntry> {
        let response = wait_for_future(py, async {
            (self.client().await?)
                .inner()
                .read_dataset_entry(ReadDatasetEntryRequest {
                    id: Some(entry_id.into()),
                })
                .await
                .map_err(to_py_err)
        })?;

        Ok(response
            .into_inner()
            .dataset
            .ok_or(PyRuntimeError::new_err("No dataset in response"))?
            .try_into()?)
    }

    pub fn update_dataset(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
        dataset_details: DatasetDetails,
    ) -> PyResult<DatasetEntry> {
        let response = wait_for_future(py, async {
            (self.client().await?)
                .inner()
                .update_dataset_entry(tonic::Request::new(
                    UpdateDatasetEntryRequest {
                        id: entry_id,
                        dataset_details,
                    }
                    .into(),
                ))
                .await
                .map_err(to_py_err)
        })?;

        Ok(response
            .into_inner()
            .dataset
            .ok_or(PyRuntimeError::new_err("No dataset in response"))?
            .try_into()?)
    }

    pub fn get_dataset_partition_ids(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
    ) -> PyResult<Vec<String>> {
        wait_for_future(py, async {
            Ok(self
                .client()
                .await?
                .get_dataset_partition_ids(entry_id)
                .await
                .map_err(to_py_err)?
                .iter()
                .map(|id| id.id.clone())
                .collect::<Vec<_>>())
        })
    }

    pub fn read_table(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<TableEntry> {
        let response = wait_for_future(py, async {
            (self.client().await?)
                .inner()
                .read_table_entry(ReadTableEntryRequest {
                    id: Some(entry_id.into()),
                })
                .await
                .map_err(to_py_err)
        })?;

        Ok(response
            .into_inner()
            .table
            .ok_or(PyRuntimeError::new_err("No table in response"))?
            .try_into()?)
    }

    pub fn get_dataset_schema(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<ArrowSchema> {
        wait_for_future(py, async {
            (self.client().await?)
                .inner()
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
        &self,
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
                .client()
                .await?
                .inner()
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

    pub fn query_tasks(&self, py: Python<'_>, task_ids: &[TaskId]) -> PyResult<RecordBatch> {
        wait_for_future(py, async {
            let request = re_protos::redap_tasks::v1alpha1::QueryTasksRequest {
                ids: task_ids.to_vec(),
            };

            let status_table = self
                .client()
                .await?
                .inner()
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
        &self,
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
                .client()
                .await?
                .inner()
                .query_tasks_on_completion(request)
                .await
                .map_err(to_py_err)?
                .into_inner();

            let mut errors: Vec<String> = Vec::new();

            // loop until all the tasks are done or the timeout is reached: both cases
            // will complete the stream
            while let Some(response) = response_stream.next().await {
                let item = response
                    .map_err(to_py_err)?
                    .data
                    .ok_or_else(|| PyValueError::new_err("received response without data"))?
                    .decode()
                    .map_err(to_py_err)?;

                // TODO(andrea): all this column unrwapping is a bit hideous. Maybe the idea of returning a dataframe rather
                // than a nicely typed object should be revisited.

                let schema = item.schema();
                if !schema.contains(&QueryTasksResponse::schema()) {
                    return Err(PyValueError::new_err(
                        "invalid schema for QueryTasksResponse",
                    ));
                }

                let col_indices = [
                    QueryTasksResponse::TASK_ID,
                    QueryTasksResponse::EXEC_STATUS,
                    QueryTasksResponse::MSGS,
                ]
                .iter()
                .map(|name| schema.index_of(name))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| PyValueError::new_err(format!("missing column: {err}")))?;

                let projected = item.project(&col_indices).map_err(to_py_err)?;

                let (task_ids, statuses, msgs) = {
                    (
                        projected
                            .column(0)
                            .try_downcast_array_ref::<arrow::array::StringArray>()
                            .map_err(to_py_err)?,
                        projected
                            .column(1)
                            .try_downcast_array_ref::<arrow::array::StringArray>()
                            .map_err(to_py_err)?,
                        projected
                            .column(2)
                            .try_downcast_array_ref::<arrow::array::StringArray>()
                            .map_err(to_py_err)?,
                    )
                };
                for i in 0..projected.num_rows() {
                    if statuses.value(i) != "success" {
                        let err = format!("task {}: {}", task_ids.value(i), msgs.value(i));
                        errors.push(err);
                    }
                }
            }

            if !errors.is_empty() {
                let msg = format!(
                    "all tasks completed, but the following errors occurred:\n{}",
                    errors.join("\n")
                );
                Err(PyValueError::new_err(msg))
            } else {
                Ok(())
            }
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_chunks_for_dataframe_query(
        &self,
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
                .client()
                .await?
                .inner()
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
