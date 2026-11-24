use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_protos::cloud::v1alpha1::{GetSegmentPropertiesRequest, ScanSegmentTableRequest};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use tokio_stream::StreamExt as _;
use tonic::codegen::{Body, StdError};

use super::RedapClient;
use crate::{ApiError, ApiErrorKind, ApiResponseStream, ApiResult};

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    /// Get the asset segments within an asset dataset.
    ///
    /// `segment_id` narrows the result to the assets that apply to that segment, honoring each
    /// asset's mode and segment list. When it is `None`, the per-asset defaults apply with no
    /// segment-specific opt-out/opt-in overrides.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_assets_for_segment(
        &mut self,
        asset_dataset_id: EntryId,
        segment_id: Option<&SegmentId>,
    ) -> ApiResult<Vec<SegmentId>> {
        let mut segments = self
            .scan_segment_table(
                asset_dataset_id,
                ScanSegmentTableRequest::with_columns([
                    ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
                ]),
            )
            .await?;
        let trace_id = segments.trace_id();

        let mut asset_segment_ids = Vec::new();
        while let Some(batch) = segments.next().await {
            let ids = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID
                .extract(&batch?)
                .map_err(|err| {
                    ApiError::deserialization_quiver_from(
                        &self.origin,
                        trace_id,
                        err,
                        "/ScanSegmentTable stream",
                    )
                })?;
            asset_segment_ids.extend(ids.into_iter_owned());
        }
        if asset_segment_ids.is_empty() {
            return Ok(asset_segment_ids);
        }

        let response = self
            .inner()
            .get_segment_properties(
                tonic::Request::new(GetSegmentPropertiesRequest {
                    segment_ids: asset_segment_ids.iter().cloned().map(Into::into).collect(),
                })
                .with_entry_id(asset_dataset_id),
            )
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/GetSegmentProperties failed"));

        match response {
            Ok(response) => {
                let mut properties = ApiResponseStream::from_tonic_response(
                    self.origin.clone(),
                    response,
                    "/GetSegmentProperties",
                );
                while let Some(response) = properties.next().await {
                    crate::asset::filter_asset_segments(
                        &self.origin,
                        &mut asset_segment_ids,
                        segment_id,
                        response?,
                    )?;
                }
            }

            // A server without `/GetSegmentProperties` cannot tell which assets apply to
            // which segment, so every asset applies.
            Err(err) if err.kind == ApiErrorKind::Unimplemented => {}

            Err(err) => return Err(err),
        }

        Ok(asset_segment_ids)
    }
}
