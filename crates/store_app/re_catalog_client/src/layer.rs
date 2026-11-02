//! Catalog layer operations.

use arrow::datatypes::Schema as ArrowSchema;
use tokio_stream::StreamExt as _;

use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ScanSegmentTableRequest;
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_protos::common::v1alpha1::TaskId;
use re_protos::common::v1alpha1::ext::SegmentId;
use re_redap_client::{ApiError, ApiResult, TraceId};
use re_types_core::LayerName;

use crate::CatalogClient;

impl CatalogClient {
    /// Read the IDs of segments currently containing this layer.
    ///
    /// A missing layer returns an empty list; a missing dataset remains an error.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_layer_segment_ids(
        &self,
        dataset_id: EntryId,
        layer: LayerName,
    ) -> ApiResult<Vec<SegmentId>> {
        let request = ScanSegmentTableRequest::with_columns([
            ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
            ScanSegmentTableDataframe::COLUMN_RERUN_LAYER_NAMES_NAME,
        ]);
        let mut stream = self
            .client()
            .await?
            .scan_segment_table(dataset_id, request)
            .await?;
        let trace_id = stream.trace_id();
        let decode_failed = |err| {
            ApiError::deserialization_quiver_from(self.origin(), trace_id, err, "/ScanSegmentTable")
        };
        let mut segment_ids = Vec::new();

        while let Some(batch) = stream.next().await {
            let batch = batch?;
            let ids = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID
                .extract(&batch)
                .map_err(decode_failed)?;
            let layers = ScanSegmentTableDataframe::COLUMN_RERUN_LAYER_NAMES
                .extract(&batch)
                .map_err(decode_failed)?;

            for (id, layers) in std::iter::zip(ids.iter_owned(), layers.iter_owned()) {
                if layers.contains(&layer) {
                    segment_ids.push(id);
                }
            }
        }

        Ok(segment_ids)
    }

    /// The union of the schemas of `layer` in every segment that has it registered.
    ///
    /// A layer that no segment has registered is not found.
    /// The order of the fields is unspecified.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_layer_schema(
        &self,
        dataset_id: EntryId,
        layer: LayerName,
    ) -> ApiResult<ArrowSchema> {
        self.client()
            .await?
            .get_layer_schema(dataset_id, &layer)
            .await?
            .ok_or_else(|| {
                ApiError::not_found(
                    self.origin(),
                    format!("layer '{layer}' not found in dataset {dataset_id}"),
                )
            })
    }

    /// Unregister exactly one segment/layer pair without deleting its source objects.
    ///
    /// Missing segments or layers are a no-op. Returns the request trace ID and task IDs;
    /// callers can use [`re_redap_client::ConnectionClient::wait_for_tasks`] to wait for completion.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn delete_segment_layer(
        &self,
        dataset_id: EntryId,
        segment_id: SegmentId,
        layer: LayerName,
    ) -> ApiResult<(Option<TraceId>, Vec<TaskId>)> {
        self.client()
            .await?
            .unregister_from_dataset(dataset_id, vec![segment_id], vec![layer], false)
            .await
    }
}
