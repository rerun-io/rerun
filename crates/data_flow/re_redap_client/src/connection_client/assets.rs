use itertools::Itertools as _;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::{
    GetAssetsForSegmentRequest, GetAssetsForSegmentResponse, GetSegmentPropertiesRequest,
};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_protos::missing_field;
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
    /// Get the asset dataset that applies to a dataset, and the asset segments within it.
    ///
    /// `segment_id` narrows the result to the assets that apply to that segment, honoring each
    /// asset's mode and segment list. When it is `None`, the per-asset defaults apply with no
    /// segment-specific opt-out/opt-in overrides.
    ///
    /// Returns `None` if the dataset cannot have assets: it has no asset dataset (no assets were
    /// ever registered for it), it is not a recording dataset, or the server has no
    /// `/GetAssetsForSegment`.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_assets_for_segment(
        &mut self,
        dataset_id: EntryId,
        segment_id: Option<SegmentId>,
    ) -> ApiResult<Option<(EntryId, Vec<SegmentId>)>> {
        let response = match self
            .inner()
            .get_assets_for_segment(
                tonic::Request::new(GetAssetsForSegmentRequest {
                    segment_id: segment_id.clone().map(Into::into),
                })
                .with_entry_id(dataset_id),
            )
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/GetAssetsForSegment"))
        {
            Ok(response) => response,
            Err(err)
                if matches!(
                    err.kind,
                    ApiErrorKind::InvalidArguments | ApiErrorKind::Unimplemented
                ) =>
            {
                return Ok(None);
            }
            Err(err) => return Err(err),
        };

        let stream = ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/GetAssetsForSegment",
        );
        let trace_id = stream.trace_id();

        let mut stream = std::pin::pin!(stream);

        let mut assets_entry = None;
        let mut asset_segment_ids = Vec::new();

        while let Some(response) = stream.next().await {
            let response = response?;

            // The asset dataset is repeated on every response, the segment ids are concatenated.
            assets_entry = Some(
                response
                    .assets_entry
                    .ok_or_else(|| {
                        ApiError::deserialization_with_source(
                            &self.origin,
                            trace_id,
                            missing_field!(GetAssetsForSegmentResponse, "assets_entry"),
                            "missing field in /GetAssetsForSegment response",
                        )
                    })?
                    .try_into()
                    .map_err(|err| {
                        ApiError::deserialization_with_source(
                            &self.origin,
                            trace_id,
                            err,
                            "failed parsing /GetAssetsForSegment response",
                        )
                    })?,
            );

            let segment_ids: Vec<SegmentId> = response
                .asset_segment_ids
                .into_iter()
                .map(TryInto::try_into)
                .try_collect()
                .map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "failed parsing /GetAssetsForSegment response",
                    )
                })?;
            asset_segment_ids.extend(segment_ids);
        }

        let Some(assets_entry) = assets_entry else {
            return Ok(None);
        };
        if !asset_segment_ids.is_empty() {
            let response = self
                .inner()
                .get_segment_properties(
                    tonic::Request::new(GetSegmentPropertiesRequest {
                        segment_ids: asset_segment_ids.iter().cloned().map(Into::into).collect(),
                    })
                    .with_entry_id(assets_entry),
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
                            segment_id.as_ref(),
                            response?,
                        )?;
                    }
                }

                // A server without `/GetSegmentProperties` cannot tell which assets apply to
                // which segment, so every asset applies.
                Err(err) if err.kind == ApiErrorKind::Unimplemented => {}

                Err(err) => return Err(err),
            }
        }

        Ok(Some((assets_entry, asset_segment_ids)))
    }
}
