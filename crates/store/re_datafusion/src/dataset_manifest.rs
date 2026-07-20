use std::collections::HashSet;
use std::sync::Arc;
use std::sync::OnceLock;

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use async_trait::async_trait;
use datafusion::catalog::TableProvider;
use datafusion::error::Result as DataFusionResult;
use datafusion::logical_expr::TableProviderFilterPushDown;
use datafusion::prelude::Expr;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::ScanDatasetManifestDataframe;
use re_protos::cloud::v1alpha1::{ScanDatasetManifestRequest, ScanDatasetManifestResponse};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_redap_client::{ApiError, ApiResult, ConnectionClient};
use tracing::instrument;

use crate::grpc_streaming_provider::{GrpcStreamProvider, GrpcStreamToTable, ScanParams};
use crate::pushdown_expressions::{
    classify_filters_for_pushdown, filters_to_pushdown_sql, pushdown_filterable_columns,
};
use crate::wasm_compat::make_future_send;

/// Public column names the server can currently filter on server-side: the scalar base columns of
/// the dataset-manifest schema.
///
/// Excluded: dynamic `property:*` columns (not supported by the server yet), binary columns like
/// `rerun_schema_sha256` (their literals don't survive the SQL round trip), and
/// `rerun_last_updated_at` — its public value is backfilled from `rerun_registration_time` for
/// legacy rows whose stored value is null, so a filter pushed to the raw server-side column would
/// wrongly drop those rows.
fn supported_filter_columns() -> &'static HashSet<String> {
    static COLUMNS: OnceLock<HashSet<String>> = OnceLock::new();
    COLUMNS.get_or_init(|| {
        let mut columns = pushdown_filterable_columns(&ScanDatasetManifestDataframe::min_schema());
        columns.remove(ScanDatasetManifestDataframe::COLUMN_RERUN_LAST_UPDATED_AT_NAME);
        columns
    })
}

//TODO(ab): deduplicate from SegmentTableProvider
#[derive(Clone)]
pub struct DatasetManifestProvider {
    client: ConnectionClient,
    dataset_id: EntryId,

    /// Captured at construction so DataFusion-spawned execution tasks can re-attach
    /// the caller's tracing span — otherwise gRPC spans below surface as root traces.
    parent_span: tracing::Span,
}

impl std::fmt::Debug for DatasetManifestProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatasetManifestProvider")
            .field("dataset_id", &self.dataset_id)
            .finish_non_exhaustive()
    }
}

impl DatasetManifestProvider {
    pub fn new(client: ConnectionClient, dataset_id: EntryId) -> Self {
        Self {
            client,
            dataset_id,
            parent_span: tracing::Span::current(),
        }
    }

    /// This is a convenience function
    pub async fn into_provider(self) -> DataFusionResult<Arc<dyn TableProvider>> {
        Ok(GrpcStreamProvider::prepare(self).await?)
    }
}

#[async_trait]
impl GrpcStreamToTable for DatasetManifestProvider {
    type GrpcStreamData = ScanDatasetManifestResponse;

    #[instrument(skip(self), err, parent = &self.parent_span)]
    async fn fetch_schema(&mut self) -> ApiResult<SchemaRef> {
        let mut client = self.client.clone();
        let dataset_id = self.dataset_id;

        Ok(Arc::new(
            make_future_send(async move { client.get_dataset_manifest_schema(dataset_id).await })
                .await?,
        ))
    }

    // TODO(ab): what `GrpcStreamToTable` attempts to simplify should probably be handled by
    // `ConnectionClient`
    #[instrument(skip(self, params), err, parent = &self.parent_span)]
    async fn send_streaming_request(
        &mut self,
        params: &ScanParams,
    ) -> ApiResult<re_redap_client::ApiResponseStream<Self::GrpcStreamData>> {
        let sql_filter = filters_to_pushdown_sql(&params.filters, supported_filter_columns())
            .unwrap_or_default();

        let request = tonic::Request::new(ScanDatasetManifestRequest {
            columns: vec![], // all of them
            sql_filter,
        })
        .with_entry_id(self.dataset_id);

        let mut client = self.client.clone();

        let response = make_future_send(async move {
            client
                .inner()
                .scan_dataset_manifest(request)
                .await
                .map_err(|err| ApiError::tonic(err, "/ScanDatasetManifest failed"))
        })
        .await?;

        Ok(re_redap_client::ApiResponseStream::from_tonic_response(
            response,
            "/ScanDatasetManifest",
        ))
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> DataFusionResult<Vec<TableProviderFilterPushDown>> {
        Ok(classify_filters_for_pushdown(
            filters,
            supported_filter_columns(),
        ))
    }

    fn process_response(
        &mut self,
        response: Self::GrpcStreamData,
        _params: &ScanParams,
    ) -> ApiResult<RecordBatch> {
        response
            .data
            .ok_or_else(|| {
                ApiError::deserialization(None, "DataFrame missing from DatasetManifest response")
            })?
            .try_into()
            .map_err(|err: re_protos::TypeConversionError| {
                ApiError::deserialization_with_source(
                    None,
                    err,
                    "failed decoding /ScanDatasetManifest response",
                )
            })
    }
}
