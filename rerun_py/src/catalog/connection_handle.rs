use std::collections::{BTreeMap, HashSet};

use arrow::array::{RecordBatch, RecordBatchIterator, RecordBatchReader};
use arrow::datatypes::Schema as ArrowSchema;
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::PyValueError;
use pyo3::{
    PyErr, PyResult, Python, create_exception, exceptions::PyConnectionError,
    exceptions::PyRuntimeError,
};
use tracing::Instrument as _;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk_store::{ChunkStore, QueryExpression};
use re_dataframe::ChunkStoreHandle;
use re_grpc_client::{ConnectionClient, ConnectionRegistryHandle};
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::{ApplicationId, EntryId, StoreId, StoreInfo, StoreKind, StoreSource, TimeInt};
use re_protos::catalog::v1alpha1::ext::DatasetDetails;
use re_protos::common::v1alpha1::ext::ScanParameters;
use re_protos::manifest_registry::v1alpha1::RegisterWithDatasetResponse;
use re_protos::redap_tasks::v1alpha1::QueryTasksResponse;
use re_protos::{
    catalog::v1alpha1::{
        EntryFilter, ReadTableEntryRequest,
        ext::{DatasetEntry, EntryDetails, TableEntry},
    },
    common::v1alpha1::{IfDuplicateBehavior, TaskId},
    frontend::v1alpha1::{
        GetChunksRequest, GetDatasetSchemaRequest, QueryDatasetRequest, RegisterWithDatasetRequest,
    },
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

impl ConnectionHandle {
    pub fn find_entries(&self, py: Python<'_>, filter: EntryFilter) -> PyResult<Vec<EntryDetails>> {
        wait_for_future(py, async {
            self.client()
                .await?
                .find_entries(filter)
                .await
                .map_err(to_py_err)
        })
    }

    pub fn delete_entry(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<()> {
        wait_for_future(py, async {
            self.client()
                .await?
                .delete_entry(entry_id)
                .await
                .map_err(to_py_err)
        })
    }

    pub fn create_dataset(&self, py: Python<'_>, name: String) -> PyResult<DatasetEntry> {
        wait_for_future(py, async {
            self.client()
                .await?
                .create_dataset_entry(name)
                .await
                .map_err(to_py_err)
        })
    }

    pub fn read_dataset(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<DatasetEntry> {
        wait_for_future(py, async {
            self.client()
                .await?
                .read_dataset_entry(entry_id)
                .await
                .map_err(to_py_err)
        })
    }

    pub fn update_dataset(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
        dataset_details: DatasetDetails,
    ) -> PyResult<DatasetEntry> {
        wait_for_future(py, async {
            self.client()
                .await?
                .update_dataset_entry(entry_id, dataset_details)
                .await
                .map_err(to_py_err)
        })
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

    // TODO(ab): migrate this to the `ConnectionClient` API.
    pub fn read_table(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<TableEntry> {
        let response = wait_for_future(py, async {
            self.client()
                .await?
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

    // TODO(ab): migrate this to the `ConnectionClient` API.
    pub fn get_dataset_schema(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<ArrowSchema> {
        wait_for_future(py, async {
            self.client()
                .await?
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
    // TODO(ab): migrate this to the `ConnectionClient` API.
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

    // TODO(ab): migrate this to the `ConnectionClient` API.
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
    // TODO(ab): migrate this to the `ConnectionClient` API.
    pub fn wait_for_tasks(
        &self,
        py: Python<'_>,
        task_ids: &[TaskId],
        timeout: std::time::Duration,
    ) -> PyResult<()> {
        use futures::StreamExt as _;

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

    // TODO(ab): migrate this to the `ConnectionClient` API.
    #[allow(clippy::too_many_arguments)]
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn get_chunks_for_dataframe_query(
        &self,
        py: Python<'_>,
        dataset_id: EntryId,
        query_expression: &QueryExpression,
        partition_ids: &[impl AsRef<str> + Sync],
    ) -> PyResult<BTreeMap<String, ChunkStoreHandle>> {
        use futures::StreamExt as _;

        let entity_paths = query_expression
            .view_contents
            .as_ref()
            .map_or(vec![], |contents| contents.keys().collect::<Vec<_>>());

        let query = query_from_query_expression(query_expression);

        let partition_ids = partition_ids
            .iter()
            .map(|id| id.as_ref().to_owned().into())
            .collect();

        let entity_paths = entity_paths
            .into_iter()
            .map(|p| (*p).clone().into())
            .collect();

        // NOTE: Do not ever run complex futures chain directly on top of `block_on`, make sure to
        // always spawn new tasks instead.
        //
        // Refer to [1] for more information.
        //
        // [1]: https://docs.rs/tokio/latest/tokio/runtime/struct.Runtime.html#non-worker-future
        let stores = wait_for_future(
            py,
            async {
                let mut client = self.client().await?;

                // First, we just kickoff the gRPC query. Nothing special here besides making sure
                // that it runs in a dedicated task, so that it can actually get scheduled properly
                // when running with `block_on`.
                let get_chunks_response_stream = tokio::spawn(async move {
                    let resp = client
                        .inner()
                        .get_chunks(GetChunksRequest {
                            dataset_id: Some(dataset_id.into()),
                            partition_ids,
                            chunk_ids: vec![],
                            entity_paths,
                            query: Some(query.into()),
                        })
                        .instrument(tracing::trace_span!("get_chunks::grpc"))
                        .await
                        .map_err(to_py_err)?
                        .into_inner();

                    Ok::<_, PyErr>(resp)
                })
                .await
                .map_err(Into::<re_grpc_client::StreamError>::into)
                .map_err(to_py_err)??;

                // Then we need to fully decode these chunks, i.e. both the transport layer (Protobuf)
                // and the app layer (Arrow).
                let mut chunk_stream =
                    re_grpc_client::get_chunks_response_to_chunk_and_partition_id(
                        get_chunks_response_stream,
                    );

                let (tx, mut rx) = tokio::sync::mpsc::channel(32); // 32 batches of chunks, not 32 chunks

                // We want the underlying HTTP2 client to keep polling on the gRPC stream as fast
                // as non-blockingly possible, which cannot happen if we just poll once in a while
                // in-between decoding phases. This results in the stream just sleeping, waiting
                // for IO to complete, way more frequently that it should.
                // We resolve that by spawning a dedicated I/O task that just polls the stream as fast as
                // the stream will allows. This way, whenever the underlying HTTP2 stream is polled, we
                // will already have pre-fetched a bunch of data for it.
                tokio::spawn(
                    async move {
                        while let Some(chunk_and_partition_id) = chunk_stream.next().await {
                            // The only possible error is the other end hanging up, which is not our problem.
                            if tx.send(chunk_and_partition_id).await.is_err() {
                                break;
                            }
                        }
                    }
                    .instrument(tracing::trace_span!("get_chunks::forward"))
                    .in_current_span(),
                );

                let mut stores = BTreeMap::default();

                while let Some(chunks_and_partition_ids) = rx.recv().await {
                    // We want to make sure to offload that compute-heavy work to the compute worker pool: it's
                    // not going to make this one single pipeline any faster, but it will prevent starvation of
                    // the Tokio runtime (which would slow down every other futures currently scheduled!).
                    stores = tokio::task::spawn_blocking({
                        // Clone the stores for mutabillity within the spawned task.
                        // Note at the end of this task we return the mutated stores and assign
                        // it back to the outer stores variable.
                        let mut stores = stores.clone();
                        move || {
                            let chunks_and_partition_ids =
                                chunks_and_partition_ids.map_err(to_py_err)?;

                            let _span = tracing::trace_span!(
                                "get_chunks::batch_insert",
                                num_chunks = chunks_and_partition_ids.len()
                            )
                            .entered();

                            for chunk_and_partition_id in chunks_and_partition_ids {
                                let (chunk, partition_id) = chunk_and_partition_id;

                                let partition_id = partition_id.ok_or_else(|| {
                                    PyValueError::new_err("Received chunk without a partition id")
                                })?;

                                let store =
                                    stores.entry(partition_id.clone()).or_insert_with(|| {
                                        let store_info = StoreInfo {
                                            application_id: ApplicationId::from(partition_id),
                                            store_id: StoreId::random(StoreKind::Recording),
                                            cloned_from: None,
                                            store_source: StoreSource::Unknown,
                                            store_version: None,
                                        };

                                        let mut store = ChunkStore::new(
                                            store_info.store_id.clone(),
                                            Default::default(),
                                        );
                                        store.set_info(store_info);
                                        ChunkStoreHandle::new(store)
                                    });

                                store
                                    .write()
                                    .insert_chunk(&std::sync::Arc::new(chunk))
                                    .map_err(to_py_err)?;
                            }

                            Ok::<_, PyErr>(stores)
                        }
                    })
                    .in_current_span()
                    .await
                    .map_err(Into::<re_grpc_client::StreamError>::into)
                    .map_err(to_py_err)??;
                }

                Ok::<_, PyErr>(stores)
            }
            .instrument(tracing::trace_span!("get_chunks"))
            .in_current_span(),
        )?;

        // Useful for debugging purposes.
        #[expect(clippy::unwrap_used)]
        if false {
            let num_chunks: usize = stores.values().map(|store| store.read().num_chunks()).sum();

            use itertools::Itertools as _;
            let stores = stores.values().map(|store| store.read()).collect_vec();
            let schemas: HashSet<_> = stores
                .iter()
                .flat_map(|store| {
                    store
                        .iter_chunks()
                        .map(|chunk| chunk.to_record_batch().unwrap().schema())
                })
                .map(|schema| {
                    // NOTE: otherwise they all would be unique, since there's the chunk ID stored in there.
                    let schema =
                        std::sync::Arc::unwrap_or_clone(schema).with_metadata(Default::default());

                    let schema_ipc = {
                        let mut schema_ipc = Vec::new();
                        arrow::ipc::writer::StreamWriter::try_new(&mut schema_ipc, &schema)
                            .unwrap();
                        schema_ipc
                    };

                    {
                        use std::hash::DefaultHasher;
                        use std::hash::Hash as _;
                        use std::hash::Hasher as _;
                        let mut hasher = DefaultHasher::new();
                        schema_ipc.hash(&mut hasher);
                        hasher.finish()
                    }
                })
                .collect();

            eprintln!(
                "num_partitions={} num_chunks={} num_unique_schemas={}",
                stores.len(),
                num_chunks,
                schemas.len()
            );
        }

        Ok(stores)
    }

    // TODO(ab): migrate this to the `ConnectionClient` API.
    pub fn get_chunk_ids_for_dataframe_query(
        &self,
        py: Python<'_>,
        dataset_id: EntryId,
        query_expression: &QueryExpression,
        partition_ids: &[impl AsRef<str> + Sync],
    ) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        use tokio_stream::StreamExt as _;

        let entity_paths = query_expression
            .view_contents
            .as_ref()
            .map_or(vec![], |contents| contents.keys().collect::<Vec<_>>());

        let query = query_from_query_expression(query_expression);

        wait_for_future(py, async {
            let response_stream = self
                .client()
                .await?
                .inner()
                .query_dataset(QueryDatasetRequest {
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
                    scan_parameters: Some(
                        ScanParameters {
                            columns: vec!["chunk_partition_id".to_owned(), "chunk_id".to_owned()],
                            ..Default::default()
                        }
                        .into(),
                    ),
                })
                .await
                .map_err(to_py_err)?
                .into_inner();

            // TODO(jleibs): Make this streaming
            let record_batches: Result<Vec<RecordBatch>, PyErr> = response_stream
                .collect::<Result<Vec<_>, _>>()
                .await
                .map_err(to_py_err)?
                .into_iter()
                .filter_map(|response| response.data)
                .map(|dataframe_part| dataframe_part.decode().map_err(to_py_err))
                .collect();

            let record_batches = record_batches?;

            // TODO(jleibs): Still need a better pattern for getting these schemas
            let first = record_batches
                .first()
                .ok_or_else(|| PyValueError::new_err("No chunks returned from query"))?;

            let schema = first.schema();

            let reader: Box<dyn RecordBatchReader + Send> = Box::new(RecordBatchIterator::new(
                record_batches.into_iter().map(Ok),
                schema,
            ));

            Ok(PyArrowType(reader))
        })
    }
}

fn query_from_query_expression(query_expression: &QueryExpression) -> Query {
    let latest_at = if query_expression.is_static() {
        Some(QueryLatestAt {
            index: None,
            at: TimeInt::STATIC.as_i64(),
            fuzzy_descriptors: vec![], // TODO(jleibs): support this
        })
    } else {
        query_expression
            .min_latest_at()
            .map(|latest_at| QueryLatestAt {
                index: Some(latest_at.timeline().to_string()),
                at: latest_at.at().as_i64(),
                fuzzy_descriptors: vec![], // TODO(jleibs): support this
            })
    };

    Query {
        latest_at,
        range: query_expression.max_range().map(|range| {
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
    }
}
