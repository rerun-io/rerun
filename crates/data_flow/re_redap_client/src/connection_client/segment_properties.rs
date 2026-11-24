use std::collections::BTreeMap;

use arrow::array::RecordBatch;
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::{
    DeleteSegmentPropertiesRequest, GetSegmentPropertiesRequest, SegmentProperties,
    SegmentPropertiesDeletion, SetSegmentPropertiesRequest,
};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_protos::missing_field;
use tokio_stream::StreamExt as _;
use tonic::codegen::{Body, StdError};

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
    /// Read the mutable properties of several segments, and the revision of each that gates
    /// writes to them.
    ///
    /// Segments without properties are left out. See [`Self::set_segment_properties`].
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_segment_properties(
        &mut self,
        entry_id: EntryId,
        segment_ids: Vec<SegmentId>,
    ) -> ApiResult<BTreeMap<SegmentId, (RecordBatch, u64)>> {
        let mut segments = BTreeMap::new();
        if segment_ids.is_empty() {
            return Ok(segments);
        }

        let (mut stream, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .get_segment_properties(
                    tonic::Request::new(GetSegmentPropertiesRequest {
                        segment_ids: segment_ids.into_iter().map(Into::into).collect(),
                    })
                    .with_entry_id(entry_id),
                )
                .await
                .map_err(|err| {
                    ApiError::tonic(&self.origin, err, "/GetSegmentProperties failed")
                })?,
        );

        while let Some(response) = stream.next().await.transpose().map_err(|err| {
            ApiError::tonic(&self.origin, err, "/GetSegmentProperties stream error")
        })? {
            for segment in response.segments {
                let Some(properties) = segment.properties else {
                    continue;
                };
                let (segment_id, batch) = segment
                    .segment_id
                    .ok_or_else(|| missing_field!(SegmentProperties, "segment_id"))
                    .and_then(SegmentId::try_from)
                    .and_then(|segment_id| {
                        RecordBatch::try_from(&properties).map(|batch| (segment_id, batch))
                    })
                    .map_err(|err| {
                        ApiError::deserialization_with_source(
                            &self.origin,
                            trace_id,
                            err,
                            "failed parsing /GetSegmentProperties response",
                        )
                    })?;
                segments.insert(segment_id, (batch, segment.revision));
            }
        }

        Ok(segments)
    }

    /// Replace one segment's mutable properties, returning the new revision.
    ///
    /// `properties` must be a single-row `RecordBatch` whose columns are named
    /// `property:<key>:<component>`, and it replaces every property on the segment without merging
    /// into the stored ones.
    ///
    /// With `expected_revision` set the write only lands if the stored revision matches, and fails
    /// with `FailedPrecondition` otherwise, leaving the stored properties untouched. `0` means "only
    /// if the segment has no properties yet". Without it the write overwrites whatever is stored.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn set_segment_properties(
        &mut self,
        entry_id: EntryId,
        segment_id: SegmentId,
        properties: &RecordBatch,
        expected_revision: Option<u64>,
    ) -> ApiResult<u64> {
        let response = self
            .inner()
            .set_segment_properties(
                tonic::Request::new(SetSegmentPropertiesRequest {
                    segment_id: Some(segment_id.into()),
                    properties: Some(properties.into()),
                    expected_revision,
                })
                .with_entry_id(entry_id),
            )
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/SetSegmentProperties failed"))?;

        Ok(response.into_inner().revision)
    }

    /// Drop the properties of several segments, whatever their revision.
    ///
    /// Dropping properties a segment doesn't have does nothing.
    /// Servers without this endpoint leave properties unchanged.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn delete_segment_properties(
        &mut self,
        entry_id: EntryId,
        segment_ids: Vec<SegmentId>,
    ) -> ApiResult<()> {
        if segment_ids.is_empty() {
            return Ok(());
        }

        let result = self
            .inner()
            .delete_segment_properties(
                tonic::Request::new(DeleteSegmentPropertiesRequest {
                    segments: segment_ids
                        .into_iter()
                        .map(|segment_id| SegmentPropertiesDeletion {
                            segment_id: Some(segment_id.into()),
                            expected_revision: None,
                        })
                        .collect(),
                })
                .with_entry_id(entry_id),
            )
            .await;

        match result {
            Ok(_) => Ok(()),
            Err(err) if err.code() == tonic::Code::Unimplemented => Ok(()),
            Err(err) => Err(ApiError::tonic(
                &self.origin,
                err,
                "/DeleteSegmentProperties failed",
            )),
        }
    }
}
