use std::collections::BTreeSet;

use arrow::array::{RecordBatch, RecordBatchIterator, RecordBatchReader};
use arrow::datatypes::{Schema as ArrowSchema, SchemaRef};
use arrow::ffi_stream::ArrowArrayStreamReader;
use arrow::pyarrow::PyArrowType;
use pyo3::exceptions::PyValueError;
use pyo3::{PyErr, PyResult, Python};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_chunk_store::QueryExpression;
use re_datafusion::query_from_query_expression;
use re_log::external::log::warn;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{
    DataSource, DatasetDetails, DatasetEntry, EntryDetails, QueryDatasetRequest,
    RegisterWithDatasetTaskDescriptor, TableEntry,
};
use re_protos::cloud::v1alpha1::{EntryFilter, QueryDatasetResponse, QueryTasksResponse};
use re_protos::common::v1alpha1::TaskId;
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, ScanParameters};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_protos::{invalid_schema, missing_field};
use re_redap_client::{ApiError, ConnectionClient, ConnectionRegistryHandle};
use tracing::Instrument as _;

use crate::catalog::table_entry::PyTableInsertModeInternal;
use crate::catalog::to_py_err;
use crate::utils::wait_for_future;

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

    pub fn connection_registry(&self) -> &ConnectionRegistryHandle {
        &self.connection_registry
    }
}

impl ConnectionHandle {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn find_entries(&self, py: Python<'_>, filter: EntryFilter) -> PyResult<Vec<EntryDetails>> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .find_entries(filter)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn delete_entry(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<()> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .delete_entry(entry_id)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn update_entry(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
        entry_details_update: re_protos::cloud::v1alpha1::ext::EntryDetailsUpdate,
    ) -> PyResult<re_protos::cloud::v1alpha1::ext::EntryDetails> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .update_entry(entry_id, entry_details_update)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn create_dataset(&self, py: Python<'_>, name: String) -> PyResult<DatasetEntry> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .create_dataset_entry(name, None)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn read_dataset(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<DatasetEntry> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .read_dataset_entry(entry_id)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn update_dataset(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
        dataset_details: DatasetDetails,
    ) -> PyResult<DatasetEntry> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .update_dataset_entry(entry_id, dataset_details)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn get_dataset_segment_ids(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
    ) -> PyResult<Vec<String>> {
        wait_for_future(
            py,
            async {
                Ok(self
                    .client()
                    .await?
                    .get_dataset_segment_ids(entry_id)
                    .await
                    .map_err(to_py_err)?
                    .iter()
                    .map(|id| id.id.clone())
                    .collect::<Vec<_>>())
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn register_table(
        &self,
        py: Python<'_>,
        name: String,
        url: url::Url,
    ) -> PyResult<TableEntry> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .register_table(name, url)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn create_table_entry(
        &self,
        py: Python<'_>,
        name: String,
        schema: SchemaRef,
        url: Option<url::Url>,
    ) -> PyResult<TableEntry> {
        let entry_id = wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .create_table_entry(&name, url, schema)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )?;

        self.read_table(py, entry_id.details.id)
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn read_table(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<TableEntry> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .read_table_entry(entry_id)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn write_table(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
        stream: ArrowArrayStreamReader,
        insert_mode: PyTableInsertModeInternal,
    ) -> PyResult<()> {
        wait_for_future(
            py,
            async {
                // Since the errors occur during streaming, we cannot let this method
                // fail without doing a collect operation. Instead, we log a warning to
                // the user.
                let stream = futures::stream::iter(stream.filter_map(move |rb| match rb {
                    Ok(rb) => Some(rb),
                    Err(err) => {
                        warn!("write_table input stream contains an error. {err}");
                        None
                    }
                }));

                self.client()
                    .await?
                    .write_table(stream, entry_id, insert_mode.into())
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn get_dataset_schema(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<ArrowSchema> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .get_dataset_schema(entry_id)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    /// Initiate registration of the provided recording URIs with a dataset and return the
    /// corresponding task descriptors.
    ///
    /// Custom layers can be specified via `recording_layers`:
    /// * When empty, this defaults to `["base"]`.
    /// * If longer than `recording_uris`, `recording_layers` will be truncated.
    /// * If shorter than `recording_uris`, `recording_layers` will be extended by repeating its last value.
    ///   I.e. an empty `recording_layers` will result in `"base"` begin repeated `len(recording_layers)` times.
    ///
    /// NOTE: The server may pool multiple registrations into a single task. The result always has
    /// the same length as the output, so task ids may be duplicated.
    #[tracing::instrument(level = "info", skip_all)]
    pub fn register_with_dataset(
        &self,
        py: Python<'_>,
        dataset_id: EntryId,
        recording_uris: Vec<String>,
        recording_layers: Vec<String>,
    ) -> PyResult<Vec<RegisterWithDatasetTaskDescriptor>> {
        let last_layer = recording_layers
            .last()
            .cloned()
            .unwrap_or_else(|| DataSource::DEFAULT_LAYER.to_owned());

        let data_sources = recording_uris
            .iter()
            .zip(
                recording_layers
                    .into_iter()
                    .chain(std::iter::repeat_with(|| last_layer.clone())),
            )
            .map(|(url, layer)| DataSource::new_rrd_layer(layer, url))
            .collect::<Result<Vec<_>, _>>()
            .map_err(to_py_err)?;

        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    //TODO(ab): expose `on_duplicate` as a method argument
                    .register_with_dataset(dataset_id, data_sources, IfDuplicateBehavior::Error)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    /// Initiate registration of all the recordings within provided object store prefix (aka directory)
    /// and return the corresponding task descriptors.
    ///
    /// A custom layer can be specified via `recordings_layer`:
    /// * When empty, this defaults to `["base"]`.
    #[tracing::instrument(level = "info", skip_all)]
    pub fn register_with_dataset_prefix(
        &self,
        py: Python<'_>,
        dataset_id: EntryId,
        recordings_prefix: String,
        recordings_layer: Option<String>,
    ) -> PyResult<Vec<RegisterWithDatasetTaskDescriptor>> {
        let layer = recordings_layer.unwrap_or_else(|| DataSource::DEFAULT_LAYER.to_owned());

        let data_source =
            DataSource::new_rrd_layer_prefix(layer, recordings_prefix).map_err(to_py_err)?;
        let data_sources = vec![data_source];

        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    //TODO(ab): expose `on_duplicate` as a method argument
                    .register_with_dataset(dataset_id, data_sources, IfDuplicateBehavior::Error)
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    #[expect(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
    pub fn do_maintenance(
        &self,
        py: Python<'_>,
        dataset_id: EntryId,
        optimize_indexes: bool,
        retrain_indexes: bool,
        compact_fragments: bool,
        cleanup_before: Option<jiff::Timestamp>,
        unsafe_allow_recent_cleanup: bool,
    ) -> PyResult<()> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .do_maintenance(
                        dataset_id,
                        optimize_indexes,
                        retrain_indexes,
                        compact_fragments,
                        cleanup_before,
                        unsafe_allow_recent_cleanup,
                    )
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn do_global_maintenance(&self, py: Python<'_>) -> PyResult<()> {
        wait_for_future(
            py,
            async {
                self.client()
                    .await?
                    .do_global_maintenance()
                    .await
                    .map_err(to_py_err)
            }
            .in_current_span(),
        )
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn query_tasks(&self, py: Python<'_>, task_ids: Vec<TaskId>) -> PyResult<RecordBatch> {
        wait_for_future(
            py,
            async {
                let status_table = self
                    .client()
                    .await?
                    .query_tasks(task_ids)
                    .await
                    .map_err(to_py_err)?
                    .dataframe_part()
                    .map_err(to_py_err)?
                    .try_into()
                    .map_err(to_py_err)?;

                Ok(status_table)
            }
            .in_current_span(),
        )
    }

    /// Wait for the provided tasks to finish.
    #[tracing::instrument(level = "info", skip_all)]
    pub fn wait_for_tasks(
        &self,
        py: Python<'_>,
        task_ids: Vec<TaskId>,
        timeout: std::time::Duration,
    ) -> PyResult<()> {
        use futures::StreamExt as _;

        wait_for_future(
            py,
            async {
                let mut response_stream = self
                    .client()
                    .await?
                    .query_tasks_on_completion(task_ids, timeout)
                    .await
                    .map_err(to_py_err)?;

                let mut errors: Vec<String> = Vec::new();

                // loop until all the tasks are done or the timeout is reached: both cases
                // will complete the stream
                while let Some(response) = response_stream.next().await {
                    let item: RecordBatch = response
                        .map_err(|err| {
                            ApiError::tonic(
                                err,
                                "failed waiting for tasks done: error receiving completion notifications",
                            )
                        })
                        .map_err(to_py_err)?
                        .data
                        .ok_or_else(|| {
                            let err = missing_field!(QueryTasksResponse, "data");
                            let err = ApiError::serialization_with_source(
                                err,
                                "failed waiting for tasks done: received item without data",
                            );
                            to_py_err(err)
                        })?
                        .try_into()
                        .map_err(to_py_err)?;

                    // TODO(andrea): all this column unwrapping is a bit hideous. Maybe the idea of returning a dataframe rather
                    // than a nicely typed object should be revisited.

                    let schema = item.schema();
                    if !schema.contains(&QueryTasksResponse::schema()) {
                        let err = invalid_schema!(QueryTasksResponse);
                        let err = ApiError::serialization_with_source(
                            err,
                            "failed waiting for tasks done: received item with invalid schema",
                        );
                        return Err(to_py_err(err));
                    }

                    let col_indices = [
                        QueryTasksResponse::FIELD_TASK_ID,
                        QueryTasksResponse::FIELD_EXEC_STATUS,
                        QueryTasksResponse::FIELD_MSGS,
                    ]
                    .iter()
                    .map(|name| schema.index_of(name))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| {
                        to_py_err(ApiError::serialization_with_source(
                            err,
                            "failed waiting for tasks done: missing column on item",
                        ))
                    })?;

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
            }
            .in_current_span(),
        )
    }

    // TODO(ab): migrate this to the `ConnectionClient` API.
    #[tracing::instrument(level = "info", skip_all)]
    pub fn get_chunk_ids_for_dataframe_query(
        &self,
        py: Python<'_>,
        dataset_id: EntryId,
        query_expression: &QueryExpression,
        segment_ids: &[impl AsRef<str> + Sync],
    ) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        use tokio_stream::StreamExt as _;

        // `/**` automatically lowers to a materialized list of entity paths on the client (and
        // it's not just blindly grabbing all the entity paths from the schemas, there's some extra
        // logic around /properties and such), which means the only way an empty ViewContents is
        // used today is because is was parsed from entity paths that didn't exist in the dataset.
        // Therefore, we are never trying to use a server-side wildcard.
        let select_all_entity_paths = false;

        let entity_paths = query_expression
            .view_contents
            .as_ref()
            .map_or(vec![], |contents| contents.keys().collect::<Vec<_>>());

        let fuzzy_descriptors: Vec<String> = query_expression
            .view_contents
            .as_ref()
            .map_or(BTreeSet::new(), |contents| {
                contents
                    .values()
                    .filter_map(|opt_set| opt_set.as_ref())
                    .flat_map(|set| set.iter().copied())
                    .collect::<BTreeSet<_>>()
            })
            .into_iter()
            .map(|ident| ident.to_string())
            .collect();

        let query = query_from_query_expression(query_expression);

        let request = QueryDatasetRequest {
            segment_ids: segment_ids
                .iter()
                .map(|id| id.as_ref().to_owned().into())
                .collect(),
            chunk_ids: vec![],
            entity_paths: entity_paths.into_iter().map(|p| (*p).clone()).collect(),
            select_all_entity_paths,
            fuzzy_descriptors,
            exclude_static_data: false,
            exclude_temporal_data: false,
            query: Some(query),
            scan_parameters: Some(ScanParameters {
                columns: vec![
                    QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID.to_owned(),
                    QueryDatasetResponse::FIELD_CHUNK_ID.to_owned(),
                ],
                ..Default::default()
            }),
        };

        wait_for_future(
            py,
            async {
                let response_stream = self
                    .client()
                    .await?
                    .inner()
                    .query_dataset(
                        tonic::Request::new(request.into())
                            .with_entry_id(dataset_id)
                            .map_err(to_py_err)?,
                    )
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
                    .map(|dataframe_part| dataframe_part.try_into().map_err(to_py_err))
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
            }
            .in_current_span(),
        )
    }
}
