use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use re_log_types::EntryId;
use re_protos::EntryName;
use re_protos::cloud::v1alpha1::ext::{
    self as cloud_ext, CreateTableEntryRequest, LanceTable, ProviderDetails,
    ReadTableEntryResponse, RegisterTableResponse, TableDetails, TableEntry, TableInsertMode,
    UpdateTableEntryRequest, UpdateTableEntryResponse,
};
use re_protos::cloud::v1alpha1::{EntryKind, ReadTableEntryRequest, WriteTableRequest};
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use tokio_stream::{Stream, StreamExt as _};
use tonic::IntoStreamingRequest as _;
use tonic::codegen::{Body, StdError};
use url::Url;

use super::RedapClient;
use super::client::TonicResponseExt;
use crate::{ApiError, ApiResult};

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    /// Get information on a table entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn read_table_entry(&mut self, entry_id: EntryId) -> ApiResult<TableEntry> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .read_table_entry(
                    tonic::Request::new(ReadTableEntryRequest {
                        id: Some(entry_id.into()),
                    })
                    .with_entry_id(entry_id),
                )
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/ReadTableEntry"))?,
        );
        let response: ReadTableEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /ReadTableEntry response",
            )
        })?;

        Ok(response.table_entry)
    }

    /// Update the details of a table entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn update_table_entry(
        &mut self,
        entry_id: EntryId,
        table_details: TableDetails,
    ) -> ApiResult<TableEntry> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .update_table_entry(tonic::Request::new(
                    UpdateTableEntryRequest {
                        id: entry_id,
                        table_details,
                    }
                    .into(),
                ))
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/UpdateTableEntry"))?,
        );
        let response: UpdateTableEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /UpdateTableEntry response",
            )
        })?;

        Ok(response.table_entry)
    }

    /// Register a foreign Lance table to a new table entry in the catalog.
    //TODO(ab): in the future, we will probably support my types of tables (parquet on S3, etc.)
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn register_table(
        &mut self,
        name: EntryName,
        url: url::Url,
    ) -> ApiResult<TableEntry> {
        let request = cloud_ext::RegisterTableRequest {
            name,
            provider_details: ProviderDetails::LanceTable(LanceTable { table_url: url }),
        };

        let origin = self.origin.clone();
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .register_table(tonic::Request::new(request.try_into().map_err(|err| {
                    ApiError::serialization_with_source(
                        &origin,
                        err,
                        "failed building /RegisterTable request",
                    )
                })?))
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/RegisterTable"))?,
        );
        let response: RegisterTableResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /RegisterTable response",
            )
        })?;

        Ok(response.table_entry)
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_table_names(&mut self) -> ApiResult<Vec<EntryName>> {
        Ok(self
            .find_entries(re_protos::cloud::v1alpha1::EntryFilter {
                // Pass both `entry_kind` (deprecated) and `entry_kinds`
                // to be compatible with old Hub versions.
                // Drop `entry_kind` when no customer has a 0.14 deployment
                // or older of Rerun Hub.
                entry_kind: Some(EntryKind::Table.into()),
                entry_kinds: vec![EntryKind::Table.into()],
                ..Default::default()
            })
            .await?
            .into_iter()
            .map(|entry| entry.name.clone())
            .collect())
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn write_table(
        &mut self,
        stream: impl Stream<Item = RecordBatch> + Send + 'static,
        table_id: EntryId,
        insert_mode: TableInsertMode,
    ) -> ApiResult {
        let insert_mode = re_protos::cloud::v1alpha1::TableInsertMode::from(insert_mode).into();
        let stream = stream
            .map(move |batch| WriteTableRequest {
                dataframe_part: Some(batch.into()),
                insert_mode,
            })
            .into_streaming_request()
            .with_entry_id(table_id);

        self.inner()
            .write_table(stream)
            .await
            .map(|_| ())
            .map_err(|err| ApiError::tonic(&self.origin, err, "/WriteTable"))
    }

    /// Create a table entry.
    ///
    /// NOTE: if `url` is provided, the caller must ensure that it is writable and yet unused.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn create_table_entry(
        &mut self,
        name: EntryName,
        url: Option<Url>,
        schema: SchemaRef,
    ) -> ApiResult<TableEntry> {
        let provider_details =
            url.map(|url| ProviderDetails::LanceTable(LanceTable { table_url: url }));
        let request = CreateTableEntryRequest {
            name,
            schema: schema.as_ref().clone(),
            provider_details,
        };

        let origin = self.origin.clone();
        let (resp, trace_id) = self
            .inner()
            .create_table_entry(tonic::Request::new(request.try_into().map_err(|err| {
                ApiError::internal_with_source(&origin, None, err, "/CreateTableEntry")
            })?))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "failed to create table"))?
            .into_inner_and_trace_id();

        resp.table
            .ok_or_else(|| {
                ApiError::deserialization(
                    &self.origin,
                    trace_id,
                    "entry ID not set in the /CreateTable response",
                )
            })?
            .try_into()
            .map_err(|err| {
                ApiError::internal_with_source(&self.origin, trace_id, err, "/CreateTable")
            })
    }
}
