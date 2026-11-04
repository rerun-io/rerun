use itertools::Itertools as _;
use re_log_types::EntryId;
use re_protos::EntryName;
use re_protos::cloud::v1alpha1::ext::{
    EntryDetails, EntryDetailsUpdate, UpdateEntryRequest, UpdateEntryResponse, WatchEventsResponse,
};
use re_protos::cloud::v1alpha1::{DeleteEntryRequest, EntryFilter, EntryKind, FindEntriesRequest};
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use tokio_stream::StreamExt as _;
use tonic::codegen::{Body, StdError};

use super::RedapClient;
use super::client::TonicResponseExt;
use crate::{ApiError, ApiResponseStream, ApiResult};

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    /// Stream catalog lifecycle events as they happen on the server.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn watch_events(&mut self) -> ApiResult<ApiResponseStream<WatchEventsResponse>> {
        let response = self
            .inner()
            .watch_events(re_protos::cloud::v1alpha1::WatchEventsRequest {
                kinds: vec![re_protos::cloud::v1alpha1::EventKind::entry()],
            })
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/WatchEvents"))?;

        let stream =
            ApiResponseStream::from_tonic_response(self.origin.clone(), response, "/WatchEvents");
        let trace_id = stream.trace_id();
        let origin = self.origin.clone();
        let stream = stream.map(move |resp| {
            resp?.try_into().map_err(|err| {
                ApiError::deserialization_with_source(
                    &origin,
                    trace_id,
                    err,
                    "failed parsing /WatchEvents response",
                )
            })
        });
        Ok(ApiResponseStream::new(
            self.origin.clone(),
            stream,
            trace_id,
        ))
    }

    /// Find all entries matching the given filter.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn find_entries(&mut self, filter: EntryFilter) -> ApiResult<Vec<EntryDetails>> {
        let (response, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .find_entries(FindEntriesRequest {
                    filter: Some(filter),
                })
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/FindEntries"))?,
        );

        response
            .entries
            .into_iter()
            .map(TryInto::try_into)
            .try_collect()
            .map_err(|err| {
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "failed parsing /FindEntries response",
                )
            })
    }

    /// Delete the provided entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn delete_entry(&mut self, entry_id: EntryId) -> ApiResult {
        self.inner()
            .delete_entry(
                tonic::Request::new(DeleteEntryRequest {
                    id: Some(entry_id.into()),
                })
                .with_entry_id(entry_id),
            )
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/DeleteEntry"))?;

        Ok(())
    }

    /// Update the provided entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn update_entry(
        &mut self,
        entry_id: EntryId,
        entry_details_update: EntryDetailsUpdate,
    ) -> ApiResult<EntryDetails> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .update_entry(
                    tonic::Request::new(
                        UpdateEntryRequest {
                            id: entry_id,
                            entry_details_update,
                        }
                        .into(),
                    )
                    .with_entry_id(entry_id),
                )
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/UpdateEntry"))?,
        );
        let response: UpdateEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /UpdateEntry response",
            )
        })?;

        Ok(response.entry_details)
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_entry_id(
        &mut self,
        entry_name: &EntryName,
        entry_kind: Option<EntryKind>,
    ) -> ApiResult<Option<EntryId>> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .find_entries(FindEntriesRequest {
                    filter: Some(EntryFilter {
                        id: None,
                        name: Some(entry_name.to_string()),
                        // Pass both `entry_kind` (deprecated) and `entry_kinds`
                        // to be compatible with old Hub versions.
                        // Drop `entry_kind` when no customer has a 0.14 deployment
                        // or older of Rerun Hub.
                        entry_kind: entry_kind.map(|kind| kind.into()),
                        entry_kinds: entry_kind.into_iter().map(|k| k as i32).collect(),
                    }),
                })
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/FindEntries"))?,
        );
        inner
            .entries
            .first()
            .and_then(|entry| entry.id)
            .map(|id| {
                EntryId::try_from(id).map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "/FindEntries",
                    )
                })
            })
            .transpose()
    }
}
