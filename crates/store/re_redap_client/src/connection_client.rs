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
        FindEntriesRequest, GetPartitionTableSchemaRequest, GetPartitionTableSchemaResponse,
        QueryDatasetRequest, QueryDatasetResponse, ReadDatasetEntryRequest, ReadTableEntryRequest,
        RegisterWithDatasetResponse, ScanPartitionTableRequest, ScanPartitionTableResponse,
        ext::{
            CreateDatasetEntryResponse, DataSource, DataSourceKind, DatasetDetails, DatasetEntry,
            EntryDetails, EntryDetailsUpdate, LanceTable, ProviderDetails as _,
            ReadDatasetEntryResponse, ReadTableEntryResponse, RegisterTableResponse,
            RegisterWithDatasetRequest, RegisterWithDatasetTaskDescriptor, TableEntry,
            UpdateDatasetEntryRequest, UpdateDatasetEntryResponse, UpdateEntryRequest,
            UpdateEntryResponse,
        },
        rerun_cloud_service_client::RerunCloudServiceClient,
    },
    common::v1alpha1::{
        ScanParameters, TaskId,
        ext::{IfDuplicateBehavior, PartitionId},
    },
    external::prost::bytes::Bytes,
    headers::RerunHeadersInjectorExt as _,
    missing_field,
};

use crate::{StreamEntryError, StreamError};

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
    ) -> Result<Vec<EntryDetails>, StreamError> {
        let result = self
            .inner()
            .find_entries(FindEntriesRequest {
                filter: Some(filter),
            })
            .await
            .map_err(|err| StreamEntryError::Find(err.into()))?
            .into_inner()
            .entries;

        Ok(result
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<EntryDetails>, _>>()?)
    }

    /// Delete the provided entry.
    pub async fn delete_entry(&mut self, entry_id: EntryId) -> Result<(), StreamError> {
        self.inner()
            .delete_entry(DeleteEntryRequest {
                id: Some(entry_id.into()),
            })
            .await
            .map_err(|err| StreamEntryError::Delete(err.into()))?;

        Ok(())
    }

    /// Update the provided entry.
    pub async fn update_entry(
        &mut self,
        entry_id: EntryId,
        entry_details_update: EntryDetailsUpdate,
    ) -> Result<EntryDetails, StreamError> {
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
            .map_err(|err| StreamEntryError::Update(err.into()))?
            .into_inner()
            .try_into()?;

        Ok(response.entry_details)
    }

    /// Create a new dataset entry.
    pub async fn create_dataset_entry(
        &mut self,
        name: String,
        entry_id: Option<EntryId>,
    ) -> Result<DatasetEntry, StreamError> {
        let response: CreateDatasetEntryResponse = self
            .inner()
            .create_dataset_entry(CreateDatasetEntryRequest {
                name: Some(name),
                id: entry_id.map(Into::into),
            })
            .await
            .map_err(|err| StreamEntryError::Create(err.into()))?
            .into_inner()
            .try_into()?;

        Ok(response.dataset)
    }

    /// Get information on a dataset entry.
    pub async fn read_dataset_entry(
        &mut self,
        entry_id: EntryId,
    ) -> Result<DatasetEntry, StreamError> {
        let response: ReadDatasetEntryResponse = self
            .inner()
            .read_dataset_entry(
                tonic::Request::new(ReadDatasetEntryRequest {})
                    .with_entry_id(entry_id)
                    .map_err(|err| StreamEntryError::InvalidId(err.into()))?,
            )
            .await
            .map_err(|err| StreamEntryError::Read(err.into()))?
            .into_inner()
            .try_into()?;

        Ok(response.dataset_entry)
    }

    /// Update the details of a dataset entry.
    pub async fn update_dataset_entry(
        &mut self,
        entry_id: EntryId,
        dataset_details: DatasetDetails,
    ) -> Result<DatasetEntry, StreamError> {
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
            .map_err(|err| StreamEntryError::Update(err.into()))?
            .into_inner()
            .try_into()?;

        Ok(response.dataset_entry)
    }

    /// Get information on a table entry.
    pub async fn read_table_entry(&mut self, entry_id: EntryId) -> Result<TableEntry, StreamError> {
        let response: ReadTableEntryResponse = self
            .inner()
            .read_table_entry(ReadTableEntryRequest {
                id: Some(entry_id.into()),
            })
            .await
            .map_err(|err| StreamEntryError::Read(err.into()))?
            .into_inner()
            .try_into()?;

        Ok(response.table_entry)
    }

    //TODO(ab): accept entry name
    pub async fn get_partition_table_schema(
        &mut self,
        entry_id: EntryId,
    ) -> Result<ArrowSchema, StreamError> {
        Ok(self
            .inner()
            .get_partition_table_schema(
                tonic::Request::new(GetPartitionTableSchemaRequest {})
                    .with_entry_id(entry_id)
                    .map_err(|err| StreamEntryError::InvalidId(err.into()))?,
            )
            .await
            .map_err(|err| StreamEntryError::GetPartitionTableSchema(err.into()))?
            .into_inner()
            .schema
            .ok_or_else(|| missing_field!(GetPartitionTableSchemaResponse, "schema"))?
            .try_into()?)
    }

    /// Get a list of partition IDs for the given dataset entry ID.
    //TODO(ab): is there a way—and a reason—to not collect and instead return a stream?
    pub async fn get_dataset_partition_ids(
        &mut self,
        entry_id: EntryId,
    ) -> Result<Vec<PartitionId>, StreamError> {
        const COLUMN_NAME: &str = ScanPartitionTableResponse::PARTITION_ID;

        let mut stream = self
            .inner()
            .scan_partition_table(
                tonic::Request::new(ScanPartitionTableRequest {
                    columns: vec![COLUMN_NAME.to_owned()],
                })
                .with_entry_id(entry_id)
                .map_err(|err| StreamEntryError::InvalidId(err.into()))?,
            )
            .await
            .map_err(|err| StreamEntryError::ReadPartitions(err.into()))?
            .into_inner();

        let mut partition_ids = Vec::new();

        while let Some(resp) = stream.next().await {
            let record_batch = resp
                .map_err(|err| StreamEntryError::ReadPartitions(err.into()))?
                .data()?
                .decode()?;

            let partition_id_col = record_batch
                .column_by_name(COLUMN_NAME)
                .ok_or_else(|| StreamError::MissingDataframeColumn(COLUMN_NAME.to_owned()))?;

            let partition_id_array =
                partition_id_col.try_downcast_array_ref::<arrow::array::StringArray>()?;

            partition_ids.extend(
                partition_id_array
                    .iter()
                    .filter_map(|opt| opt.map(|s| PartitionId::new(s.to_owned()))),
            );
        }

        Ok(partition_ids)
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
    ) -> Result<FetchChunksResponseStream, StreamError> {
        let fields_of_interest = [
            QueryDatasetResponse::PARTITION_ID,
            QueryDatasetResponse::CHUNK_ID,
            QueryDatasetResponse::PARTITION_LAYER,
            QueryDatasetResponse::CHUNK_KEY,
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();

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
                columns: fields_of_interest,
                ..Default::default()
            }),
        };

        let response_stream = self
            .inner()
            .query_dataset(
                tonic::Request::new(query_request)
                    .with_entry_id(dataset_id)
                    .map_err(|err| crate::StreamPartitionError::StreamingChunks(err.into()))?,
            )
            .await
            .map_err(|err| crate::StreamPartitionError::StreamingChunks(err.into()))?
            .into_inner();

        let chunk_info_batches = response_stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| crate::StreamPartitionError::StreamingChunks(err.into()))?
            .into_iter()
            .map(|resp| {
                resp.data.ok_or(crate::StreamError::MissingData(
                    "missing data in QueryDatasetResponse".to_owned(),
                ))
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
            .map_err(|err| crate::StreamPartitionError::StreamingChunks(err.into()))?
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
    ) -> Result<Vec<RegisterWithDatasetTaskDescriptor>, StreamError> {
        let req = tonic::Request::new(RegisterWithDatasetRequest {
            data_sources,
            on_duplicate,
        })
        .with_entry_id(dataset_id)
        .map_err(|err| StreamEntryError::InvalidId(err.into()))?;

        let response = self
            .inner()
            .register_with_dataset(req.map(Into::into))
            .await
            .map_err(|err| StreamEntryError::RegisterData(err.into()))?
            .into_inner()
            .data
            .ok_or_else(|| missing_field!(RegisterWithDatasetResponse, "data"))?
            .decode()?;

        // TODO(andrea): why is the schema completely off?
        #[expect(clippy::overly_complex_bool_expr)]
        if false
            && !response
                .schema()
                .contains(&RegisterWithDatasetResponse::schema())
        {
            return Err(StreamError::MissingDataframeColumn(
                "invalid schema for RegisterWithDatasetResponse".to_owned(),
            ));
        }

        let get_string_array = |column_name: &str| {
            response
                .column_by_name(column_name)
                .and_then(|column| {
                    column
                        .try_downcast_array_ref::<arrow::array::StringArray>()
                        .ok()
                })
                .ok_or_else(|| StreamError::MissingDataframeColumn(column_name.to_owned()))
        };

        let partition_id_column = get_string_array(RegisterWithDatasetResponse::PARTITION_ID)?;
        let partition_type_column = DataSourceKind::many_from_arrow(
            response
                .column_by_name(RegisterWithDatasetResponse::PARTITION_TYPE)
                .ok_or_else(|| {
                    StreamError::MissingDataframeColumn(
                        RegisterWithDatasetResponse::PARTITION_TYPE.to_owned(),
                    )
                })?,
        )?;
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
                            StreamError::MissingData("Unexpected null partition id".to_owned())
                        })?
                        .to_owned(),
                ),
                partition_type,
                storage_url: url::Url::parse(storage_url.ok_or_else(|| {
                    StreamError::MissingData("Unexpected null storage_url".to_owned())
                })?)
                .map_err(TypeConversionError::UrlParseError)?,
                task_id: TaskId {
                    id: task_id
                        .ok_or_else(|| {
                            StreamError::MissingData("Unexpected null task_id".to_owned())
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
    ) -> Result<TableEntry, StreamError> {
        let request = re_protos::cloud::v1alpha1::ext::RegisterTableRequest {
            name,
            provider_details: LanceTable { table_url: url }.try_as_any()?,
        };

        let response: RegisterTableResponse = self
            .inner()
            .register_table(tonic::Request::new(request.into()))
            .await
            .map_err(|err| StreamEntryError::RegisterTable(err.into()))?
            .into_inner()
            .try_into()?;

        Ok(response.table_entry)
    }

    #[allow(clippy::fn_params_excessive_bools)]
    pub async fn do_maintenance(
        &mut self,
        dataset_id: EntryId,
        optimize_indexes: bool,
        retrain_indexes: bool,
        compact_fragments: bool,
        cleanup_before: Option<jiff::Timestamp>,
        unsafe_allow_recent_cleanup: bool,
    ) -> Result<(), StreamError> {
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
                .map_err(|err| StreamEntryError::InvalidId(err.into()))?,
            )
            .await
            .map_err(|err| StreamEntryError::Maintenance(err.into()))?;

        Ok(())
    }

    pub async fn do_global_maintenance(&mut self) -> Result<(), StreamError> {
        self.inner()
            .do_global_maintenance(tonic::Request::new(
                re_protos::cloud::v1alpha1::DoGlobalMaintenanceRequest {},
            ))
            .await
            .map_err(|err| StreamEntryError::Maintenance(err.into()))?;

        Ok(())
    }

    pub async fn get_table_names(&mut self) -> Result<Vec<String>, StreamError> {
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
}
