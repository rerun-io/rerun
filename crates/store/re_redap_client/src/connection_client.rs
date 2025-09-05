use tokio_stream::StreamExt as _;
use tonic::codegen::{Body, StdError};

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_log_encoding::codec::wire::decoder::Decode as _;
use re_log_types::EntryId;
use re_protos::external::prost::bytes::Bytes;
use re_protos::{
    TypeConversionError,
    cloud::v1alpha1::{
        CreateDatasetEntryRequest, DeleteEntryRequest, EntryFilter, FindEntriesRequest,
        ReadDatasetEntryRequest, ReadTableEntryRequest,
        ext::{
            CreateDatasetEntryResponse, DatasetDetails, DatasetEntry, EntryDetails,
            EntryDetailsUpdate, LanceTable, ProviderDetails as _, ReadDatasetEntryResponse,
            ReadTableEntryResponse, RegisterTableResponse, TableEntry, UpdateDatasetEntryRequest,
            UpdateDatasetEntryResponse, UpdateEntryRequest, UpdateEntryResponse,
        },
    },
    cloud::v1alpha1::{
        RegisterWithDatasetResponse, ScanPartitionTableResponse,
        ext::{DataSource, DataSourceKind, RegisterWithDatasetTaskDescriptor},
    },
    cloud::v1alpha1::{
        ext::{RegisterWithDatasetRequest, ScanPartitionTableRequest},
        rerun_cloud_service_client::RerunCloudServiceClient,
    },
    common::v1alpha1::{
        TaskId,
        ext::{IfDuplicateBehavior, IfMissingBehavior, PartitionId, ScanParameters},
    },
    headers::RerunHeadersInjectorExt as _,
    missing_field,
};

use crate::StreamError;

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
            .await?
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
            .await?;

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
            .await?
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
            .await?
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
                tonic::Request::new(ReadDatasetEntryRequest {}).with_entry_id(entry_id)?,
            )
            .await?
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
            .await?
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
            .await?
            .into_inner()
            .try_into()?;

        Ok(response.table_entry)
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
                tonic::Request::new(
                    ScanPartitionTableRequest {
                        scan_parameters: Some(ScanParameters {
                            columns: vec![COLUMN_NAME.to_owned()],
                            on_missing_columns: IfMissingBehavior::Error,
                            ..Default::default()
                        }),
                    }
                    .into(),
                )
                .with_entry_id(entry_id)?,
            )
            .await?
            .into_inner();

        let mut partition_ids = Vec::new();

        while let Some(resp) = stream.next().await {
            let record_batch = resp?.data()?.decode()?;

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
        .with_entry_id(dataset_id)?;

        let response = self
            .inner()
            .register_with_dataset(req.map(Into::into))
            .await?
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
            .await?
            .into_inner()
            .try_into()?;

        Ok(response.table_entry)
    }

    #[allow(clippy::fn_params_excessive_bools)]
    pub async fn do_maintenance(
        &mut self,
        dataset_id: EntryId,
        build_scalar_indexes: bool,
        compact_fragments: bool,
        cleanup_before: Option<jiff::Timestamp>,
        unsafe_allow_recent_cleanup: bool,
    ) -> Result<(), StreamError> {
        self.inner()
            .do_maintenance(tonic::Request::new(
                re_protos::cloud::v1alpha1::ext::DoMaintenanceRequest {
                    dataset_id: Some(dataset_id.into()),
                    build_scalar_indexes,
                    compact_fragments,
                    cleanup_before,
                    unsafe_allow_recent_cleanup,
                }
                .into(),
            ))
            .await?;

        Ok(())
    }
}
