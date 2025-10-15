use arrow::datatypes::Schema as ArrowSchema;
use tokio_stream::{Stream, StreamExt as _};
use tonic::codegen::{Body, StdError};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::EntryId;
use re_protos::{
    TypeConversionError,
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, DeleteEntryRequest, EntryFilter, EntryKind, FetchChunksRequest,
        FindEntriesRequest, GetDatasetManifestSchemaRequest, GetDatasetManifestSchemaResponse,
        GetDatasetSchemaRequest, GetPartitionTableSchemaRequest, GetPartitionTableSchemaResponse,
        QueryDatasetRequest, QueryDatasetResponse, QueryTasksOnCompletionResponse,
        QueryTasksResponse, ReadDatasetEntryRequest, ReadTableEntryRequest,
        RegisterWithDatasetResponse, ScanPartitionTableRequest, ScanPartitionTableResponse,
        ext::{
            CreateDatasetEntryResponse, DataSource, DataSourceKind, DatasetDetails, DatasetEntry,
            EntryDetails, EntryDetailsUpdate, LanceTable, ProviderDetails as _,
            QueryTasksOnCompletionRequest, QueryTasksRequest, ReadDatasetEntryResponse,
            ReadTableEntryResponse, RegisterTableResponse, RegisterWithDatasetRequest,
            RegisterWithDatasetTaskDescriptor, TableEntry, UpdateDatasetEntryRequest,
            UpdateDatasetEntryResponse, UpdateEntryRequest, UpdateEntryResponse,
        },
        rerun_cloud_service_client::RerunCloudServiceClient,
    },
    common::v1alpha1::{
        ScanParameters, TaskId,
        ext::{IfDuplicateBehavior, PartitionId},
    },
    external::prost::bytes::Bytes,
    headers::RerunHeadersInjectorExt as _,
    invalid_schema, missing_column, missing_field,
};

use crate::ApiError;

pub type FetchChunksResponseStream = std::pin::Pin<
    Box<
        dyn Stream<Item = Result<re_protos::cloud::v1alpha1::FetchChunksResponse, tonic::Status>>
            + Send,
    >,
>;

/// Expose an ergonomic API over the gRPC redap client.
///
/// Implementation note: this type is generic so that it can be used with several client types. This
/// is useful for other projects which might have different type (e.g. due to instrumentation).
/// For the viewer, use [`crate::ConnectionClient`].
//TODO(ab): this should NOT be `Clone`, to discourage callsites from holding on to a client for too
//long. However we have a bunch of places that needs to be fixed before we can do that.
#[derive(Debug, Clone)]
pub struct GenericConnectionClient<T>(RerunCloudServiceClient<T>);

impl<T> GenericConnectionClient<T> {
    /// Create a new [`Self`].
    ///
    /// This should not be used in the viewer, use [`crate::ConnectionRegistryHandle::client`]
    /// instead.
    pub fn new(client: RerunCloudServiceClient<T>) -> Self {
        Self(client)
    }

    /// Get a mutable reference to the underlying `RedapClient`.
    //TODO(#10188): this should disappear once we have wrapper for all endpoints and the client code
    //is using them.
    pub fn inner(&mut self) -> &mut RerunCloudServiceClient<T> {
        &mut self.0
    }
}

// ---

impl<T> GenericConnectionClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body>,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    /// Find all entries matching the given filter.
    pub async fn find_entries(
        &mut self,
        filter: EntryFilter,
    ) -> Result<Vec<EntryDetails>, ApiError> {
        let result = self
            .inner()
            .find_entries(FindEntriesRequest {
                filter: Some(filter),
            })
            .await
            .map_err(|err| ApiError::tonic(err, "/FindEntries failed"))?
            .into_inner()
            .entries;

        result
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<EntryDetails>, _>>()
            .map_err(|err| ApiError::serialization(err, "failed parsing /FindEntries response"))
    }

    /// Delete the provided entry.
    pub async fn delete_entry(&mut self, entry_id: EntryId) -> Result<(), ApiError> {
        self.inner()
            .delete_entry(DeleteEntryRequest {
                id: Some(entry_id.into()),
            })
            .await
            .map_err(|err| ApiError::tonic(err, "/DeleteEntry failed"))?;

        Ok(())
    }

    /// Update the provided entry.
    pub async fn update_entry(
        &mut self,
        entry_id: EntryId,
        entry_details_update: EntryDetailsUpdate,
    ) -> Result<EntryDetails, ApiError> {
        let response: UpdateEntryResponse = self
            .inner()
            .update_entry(tonic::Request::new(
                UpdateEntryRequest {
                    id: entry_id,
                    entry_details_update,
                }
                .into(),
            ))
            .await
            .map_err(|err| ApiError::tonic(err, "/UpdateEntry failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| ApiError::serialization(err, "failed parsing /UpdateEntry response"))?;

        Ok(response.entry_details)
    }

    /// Get the Arrow schema for a dataset entry.
    pub async fn get_dataset_schema(&mut self, entry_id: EntryId) -> Result<ArrowSchema, ApiError> {
        self.inner()
            .get_dataset_schema(
                tonic::Request::new(GetDatasetSchemaRequest {})
                    .with_entry_id(entry_id)
                    .map_err(|err| {
                        ApiError::tonic(err, "failed building /GetDatasetSchema request")
                    })?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/GetDatasetSchema failed"))?
            .into_inner()
            .schema()
            .map_err(|err| {
                ApiError::serialization(err, "failed parsing /GetDatasetSchema response")
            })
    }

    /// Create a new dataset entry.
    pub async fn create_dataset_entry(
        &mut self,
        name: String,
        entry_id: Option<EntryId>,
    ) -> Result<DatasetEntry, ApiError> {
        let response: CreateDatasetEntryResponse = self
            .inner()
            .create_dataset_entry(CreateDatasetEntryRequest {
                name: Some(name),
                id: entry_id.map(Into::into),
            })
            .await
            .map_err(|err| ApiError::tonic(err, "/CreateDatasetEntry failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization(err, "failed parsing /CreateDatasetEntry response")
            })?;

        Ok(response.dataset)
    }

    /// Get information on a dataset entry.
    pub async fn read_dataset_entry(
        &mut self,
        entry_id: EntryId,
    ) -> Result<DatasetEntry, ApiError> {
        let response: ReadDatasetEntryResponse = self
            .inner()
            .read_dataset_entry(
                tonic::Request::new(ReadDatasetEntryRequest {})
                    .with_entry_id(entry_id)
                    .map_err(|err| {
                        ApiError::tonic(err, "failed building /ReadDatasetEntry request")
                    })?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/ReadDatasetEntry failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization(err, "failed parsing /ReadDatasetEntry response")
            })?;

        Ok(response.dataset_entry)
    }

    /// Update the details of a dataset entry.
    pub async fn update_dataset_entry(
        &mut self,
        entry_id: EntryId,
        dataset_details: DatasetDetails,
    ) -> Result<DatasetEntry, ApiError> {
        let response: UpdateDatasetEntryResponse = self
            .inner()
            .update_dataset_entry(tonic::Request::new(
                UpdateDatasetEntryRequest {
                    id: entry_id,
                    dataset_details,
                }
                .into(),
            ))
            .await
            .map_err(|err| ApiError::tonic(err, "/UpdateDatasetEntry failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization(err, "failed parsing /UpdateDatasetEntry response")
            })?;

        Ok(response.dataset_entry)
    }

    /// Get information on a table entry.
    pub async fn read_table_entry(&mut self, entry_id: EntryId) -> Result<TableEntry, ApiError> {
        let response: ReadTableEntryResponse = self
            .inner()
            .read_table_entry(ReadTableEntryRequest {
                id: Some(entry_id.into()),
            })
            .await
            .map_err(|err| ApiError::tonic(err, "/ReadTableEntry failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization(err, "failed parsing /ReadTableEntry response")
            })?;

        Ok(response.table_entry)
    }

    //TODO(ab): accept entry name
    pub async fn get_partition_table_schema(
        &mut self,
        entry_id: EntryId,
    ) -> Result<ArrowSchema, ApiError> {
        self.inner()
            .get_partition_table_schema(
                tonic::Request::new(GetPartitionTableSchemaRequest {})
                    .with_entry_id(entry_id)
                    .map_err(|err| {
                        ApiError::tonic(err, "failed building /GetPartitionTableSchema request")
                    })?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "GetPartitionTableSchema failed"))?
            .into_inner()
            .schema
            .ok_or_else(|| {
                let err = missing_field!(GetPartitionTableSchemaResponse, "schema");
                ApiError::serialization(err, "missing field in /GetPartitionTableSchema response")
            })?
            .try_into()
            .map_err(|err| {
                ApiError::serialization(err, "failed parsing /GetPartitionTableSchema response")
            })
    }

    /// Get a list of partition IDs for the given dataset entry ID.
    //TODO(ab): is there a way—and a reason—to not collect and instead return a stream?
    pub async fn get_dataset_partition_ids(
        &mut self,
        entry_id: EntryId,
    ) -> Result<Vec<PartitionId>, ApiError> {
        const COLUMN_NAME: &str = ScanPartitionTableResponse::FIELD_PARTITION_ID;

        let mut stream = self
            .inner()
            .scan_partition_table(
                tonic::Request::new(ScanPartitionTableRequest {
                    columns: vec![COLUMN_NAME.to_owned()],
                })
                .with_entry_id(entry_id)
                .map_err(|err| {
                    ApiError::tonic(err, "failed building /ScanPartitionTable request")
                })?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/ScanPartitionTable failed"))?
            .into_inner();

        let mut partition_ids = Vec::new();

        while let Some(resp) = stream.next().await {
            let record_batch = resp
                .map_err(|err| {
                    ApiError::tonic(err, "failed receiving item from /ScanPartitionTable stream")
                })?
                .data()
                .map_err(|err| {
                    ApiError::serialization(
                        err,
                        "failed parsing item from /ScanPartitionTable stream",
                    )
                })?
                .decode()
                .map_err(|err| {
                    ApiError::serialization(
                        err,
                        "failed decoding item from /ScanPartitionTable stream",
                    )
                })?;

            let partition_id_col = record_batch.column_by_name(COLUMN_NAME).ok_or_else(|| {
                let err = missing_column!(ScanPartitionTableResponse, COLUMN_NAME);
                ApiError::serialization(
                    err,
                    "missing column from item in /ScanPartitionTable stream",
                )
            })?;

            let partition_id_array = partition_id_col
                .try_downcast_array_ref::<arrow::array::StringArray>()
                .map_err(|err| {
                    ApiError::serialization(
                        err,
                        "unexpected types in item in /ScanPartitionTable stream",
                    )
                })?;

            partition_ids.extend(
                partition_id_array
                    .iter()
                    .filter_map(|opt| opt.map(|s| PartitionId::new(s.to_owned()))),
            );
        }

        Ok(partition_ids)
    }

    //TODO(ab): accept entry name
    pub async fn get_dataset_manifest_schema(
        &mut self,
        entry_id: EntryId,
    ) -> Result<ArrowSchema, ApiError> {
        self.inner()
            .get_dataset_manifest_schema(
                tonic::Request::new(GetDatasetManifestSchemaRequest {})
                    .with_entry_id(entry_id)
                    .map_err(|err| {
                        ApiError::tonic(err, "failed building /GetDatasetManifestSchema request")
                    })?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/GetDatasetManifestSchema failed"))?
            .into_inner()
            .schema
            .ok_or_else(|| {
                let err = missing_field!(GetDatasetManifestSchemaResponse, "schema");
                ApiError::serialization(err, "missing field in /GetDatasetManifestSchema response")
            })?
            .try_into()
            .map_err(|err| {
                ApiError::serialization(err, "failed parsing /GetDatasetManifestSchema response")
            })
    }

    /// Fetches all chunks for a specified partition. You can include/exclude static/temporal chunks.
    /// TODO(zehiko) We should also expose query and fetch separately
    pub async fn fetch_partition_chunks(
        &mut self,
        dataset_id: EntryId,
        partition_id: PartitionId,
        exclude_static_data: bool,
        exclude_temporal_data: bool,
        query: Option<re_protos::cloud::v1alpha1::Query>,
    ) -> Result<FetchChunksResponseStream, ApiError> {
        let query_request = QueryDatasetRequest {
            partition_ids: vec![partition_id.into()],
            chunk_ids: vec![],
            entity_paths: vec![],
            select_all_entity_paths: true,
            fuzzy_descriptors: vec![],
            exclude_static_data,
            exclude_temporal_data,
            query,
            scan_parameters: Some(ScanParameters {
                columns: FetchChunksRequest::required_column_names(),
                ..Default::default()
            }),
        };

        let response_stream = self
            .inner()
            .query_dataset(
                tonic::Request::new(query_request)
                    .with_entry_id(dataset_id)
                    .map_err(|err| ApiError::tonic(err, "failed building /QueryDataset request"))?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/QueryDataset failed"))?
            .into_inner();

        let chunk_info_batches = response_stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                ApiError::tonic(
                    err,
                    "failed receiving items in /QueryDataset response stream",
                )
            })?
            .into_iter()
            .map(|resp| {
                resp.data.ok_or_else(|| {
                    let err = missing_field!(QueryDatasetResponse, "data");
                    ApiError::serialization(
                        err,
                        "missing field in item in /QueryDataset response stream",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        if chunk_info_batches.is_empty() {
            let empty_stream = tokio_stream::empty();
            return Ok(Box::pin(empty_stream));
        }

        let fetch_chunks_request = FetchChunksRequest {
            chunk_infos: chunk_info_batches,
        };

        let fetch_chunks_response_stream = self
            .inner()
            .fetch_chunks(fetch_chunks_request)
            .await
            .map_err(|err| ApiError::tonic(err, "/FetchChunks failed"))?
            .into_inner();

        Ok(Box::pin(fetch_chunks_response_stream))
    }

    /// Initiate registration of the provided recording URIs with a dataset and return the
    /// corresponding task descriptors.
    ///
    /// NOTE: The server may pool multiple registrations into a single task. The result always has
    /// the same length as the output, so task ids may be duplicated.
    pub async fn register_with_dataset(
        &mut self,
        dataset_id: EntryId,
        data_sources: Vec<DataSource>,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<Vec<RegisterWithDatasetTaskDescriptor>, ApiError> {
        let req = tonic::Request::new(RegisterWithDatasetRequest {
            data_sources,
            on_duplicate,
        })
        .with_entry_id(dataset_id)
        .map_err(|err| ApiError::tonic(err, "failed building /RegisterWithDataset request"))?;

        let response = self
            .inner()
            .register_with_dataset(req.map(Into::into))
            .await
            .map_err(|err| ApiError::tonic(err, "/RegisterWithDataset failed"))?
            .into_inner()
            .data
            .ok_or_else(|| {
                let err = missing_field!(RegisterWithDatasetResponse, "data");
                ApiError::serialization(err, "missing field in /RegisterWithDataset response")
            })?
            .decode()
            .map_err(|err| {
                ApiError::serialization(err, "failed decoding /RegisterWithDataset response")
            })?;

        // TODO(andrea): why is the schema completely off?
        #[expect(clippy::overly_complex_bool_expr)]
        if false
            && !response
                .schema()
                .contains(&RegisterWithDatasetResponse::schema())
        {
            let err = invalid_schema!(RegisterWithDatasetResponse);
            return Err(ApiError::serialization(
                err,
                "invalid schema in /RegisterWithDataset response",
            ));
        }

        let get_string_array = |column_name: &'static str| {
            response
                .column_by_name(column_name)
                .and_then(|column| {
                    column
                        .try_downcast_array_ref::<arrow::array::StringArray>()
                        .ok()
                })
                .ok_or_else(|| {
                    let err = missing_column!(RegisterWithDatasetResponse, column_name);
                    ApiError::serialization(err, "missing column in /RegisterWithDataset response")
                })
        };

        let partition_id_column = get_string_array(RegisterWithDatasetResponse::PARTITION_ID)?;
        let partition_type_column = DataSourceKind::many_from_arrow(
            response
                .column_by_name(RegisterWithDatasetResponse::PARTITION_TYPE)
                .ok_or_else(|| {
                    let err = missing_column!(
                        RegisterWithDatasetResponse,
                        RegisterWithDatasetResponse::PARTITION_TYPE
                    );
                    ApiError::serialization(err, "missing column in /RegisterWithDataset response")
                })?,
        )
        .map_err(|err| {
            ApiError::serialization(err, "failed parsing /RegisterWithDataset response")
        })?;
        let storage_url_column = get_string_array(RegisterWithDatasetResponse::STORAGE_URL)?;
        let task_id_column = get_string_array(RegisterWithDatasetResponse::TASK_ID)?;

        itertools::izip!(
            partition_id_column,
            partition_type_column,
            storage_url_column,
            task_id_column,
        )
        .map(|(partition_id, partition_type, storage_url, task_id)| {
            Ok(RegisterWithDatasetTaskDescriptor {
                partition_id: PartitionId::new(
                    partition_id
                        .ok_or_else(|| {
                            let err = missing_field!(RegisterWithDatasetResponse, "partition_id");
                            ApiError::serialization(
                                err,
                                "missing field in /RegisterWithDataset response",
                            )
                        })?
                        .to_owned(),
                ),
                partition_type,
                storage_url: url::Url::parse(storage_url.ok_or_else(|| {
                    let err = missing_field!(RegisterWithDatasetResponse, "storage_url");
                    ApiError::serialization(err, "missing field in /RegisterWithDataset response")
                })?)
                .map_err(|err| {
                    ApiError::serialization(
                        TypeConversionError::UrlParseError(err),
                        "failed to parse /RegisterWithDataset response",
                    )
                })?,
                task_id: TaskId {
                    id: task_id
                        .ok_or_else(|| {
                            let err = missing_field!(RegisterWithDatasetResponse, "task_id");
                            ApiError::serialization(
                                err,
                                "missing field in /RegisterWithDataset response",
                            )
                        })?
                        .to_owned(),
                },
            })
        })
        .collect()
    }

    /// Register a foreign Lance table to a new table entry in the catalog.
    //TODO(ab): in the future, we will probably support my types of tables (parquet on S3, etc.)
    pub async fn register_table(
        &mut self,
        name: String,
        url: url::Url,
    ) -> Result<TableEntry, ApiError> {
        let request = re_protos::cloud::v1alpha1::ext::RegisterTableRequest {
            name,
            provider_details: LanceTable { table_url: url }.try_as_any().map_err(|err| {
                ApiError::serialization(err, "failed building /RegisterTable request")
            })?,
        };

        let response: RegisterTableResponse = self
            .inner()
            .register_table(tonic::Request::new(request.into()))
            .await
            .map_err(|err| ApiError::tonic(err, "/RegisterTable failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization(err, "failed parsing /RegisterTable response")
            })?;

        Ok(response.table_entry)
    }

    #[expect(clippy::fn_params_excessive_bools)]
    pub async fn do_maintenance(
        &mut self,
        dataset_id: EntryId,
        optimize_indexes: bool,
        retrain_indexes: bool,
        compact_fragments: bool,
        cleanup_before: Option<jiff::Timestamp>,
        unsafe_allow_recent_cleanup: bool,
    ) -> Result<(), ApiError> {
        self.inner()
            .do_maintenance(
                tonic::Request::new(
                    re_protos::cloud::v1alpha1::ext::DoMaintenanceRequest {
                        optimize_indexes,
                        retrain_indexes,
                        compact_fragments,
                        cleanup_before,
                        unsafe_allow_recent_cleanup,
                    }
                    .into(),
                )
                .with_entry_id(dataset_id)
                .map_err(|err| ApiError::tonic(err, "failed building /DoMaintenance request"))?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/DoMaintenance failed"))?;

        Ok(())
    }

    pub async fn do_global_maintenance(&mut self) -> Result<(), ApiError> {
        self.inner()
            .do_global_maintenance(tonic::Request::new(
                re_protos::cloud::v1alpha1::DoGlobalMaintenanceRequest {},
            ))
            .await
            .map_err(|err| ApiError::tonic(err, "/DoGlobalMaintenance failed"))?;

        Ok(())
    }

    pub async fn get_table_names(&mut self) -> Result<Vec<String>, ApiError> {
        Ok(self
            .find_entries(re_protos::cloud::v1alpha1::EntryFilter {
                entry_kind: Some(EntryKind::Table.into()),
                ..Default::default()
            })
            .await?
            .into_iter()
            .map(|entry| entry.name.clone())
            .collect())
    }

    // -- Tasks API --
    pub async fn query_tasks_on_completion(
        &mut self,
        task_ids: Vec<TaskId>,
        timeout: std::time::Duration,
    ) -> Result<tonic::Streaming<QueryTasksOnCompletionResponse>, ApiError> {
        let q = QueryTasksOnCompletionRequest { task_ids, timeout };
        let response = self
            .inner()
            .query_tasks_on_completion(tonic::Request::new(q.try_into().map_err(|err| {
                ApiError::serialization(err, "failed building /QueryTasksOnCompletion request")
            })?))
            .await
            .map_err(|err| ApiError::tonic(err, "/QueryTasksOnCompletion failed"))?
            .into_inner();
        Ok(response)
    }

    pub async fn query_tasks(
        &mut self,
        task_ids: Vec<TaskId>,
    ) -> Result<QueryTasksResponse, ApiError> {
        let q = QueryTasksRequest { task_ids };
        let response = self
            .inner()
            .query_tasks(tonic::Request::new(q.try_into().map_err(|err| {
                ApiError::serialization(err, "failed building /QueryTasks request")
            })?))
            .await
            .map_err(|err| ApiError::tonic(err, "/QueryTasks failed"))?
            .into_inner();
        Ok(response)
    }
}
