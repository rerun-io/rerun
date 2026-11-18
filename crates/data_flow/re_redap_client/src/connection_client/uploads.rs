use re_protos::cloud::v1alpha1::ext::{self as cloud_ext, GetWriteAccessGrantResponse};
use re_protos::external::prost::bytes::Bytes;
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
    /// Lists configured locations, sorted by alias, without testing storage access.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_storage_locations(&mut self) -> ApiResult<cloud_ext::StorageLocations> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .get_storage_locations(re_protos::cloud::v1alpha1::GetStorageLocationsRequest {})
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/GetStorageLocations failed"))?,
        );
        let locations = inner
            .storage_locations
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "failed parsing /GetStorageLocations response",
                )
            })?;
        let default_alias =
            cloud_ext::StorageLocationAlias::try_new(inner.default_alias).map_err(|err| {
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "failed parsing /GetStorageLocations default alias",
                )
            })?;
        Ok(cloud_ext::StorageLocations {
            locations,
            default_alias,
        })
    }

    /// Requests authorization to write one object.
    ///
    /// Writing the object does not register it: redeem the grant, then pass the returned storage
    /// URL to [`Self::register_with_dataset`], which is a separate operation and may happen much
    /// later.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_write_access_grant(
        &mut self,
        request: cloud_ext::GetWriteAccessGrantRequest,
    ) -> ApiResult<GetWriteAccessGrantResponse> {
        // TODO(RR-6055): Negotiate explicit-location support; older servers ignore the selector.
        if request.storage_location.is_some() {
            return Err(ApiError::unimplemented(
                &self.origin,
                "explicit storage locations are not supported for uploads",
            ));
        }

        let supported_upload_methods = request.supported_upload_methods.clone();
        let signing = request.signing;
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .get_write_access_grant(
                    re_protos::cloud::v1alpha1::GetWriteAccessGrantRequest::from(request),
                )
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/GetWriteAccessGrant failed"))?,
        );
        let response: GetWriteAccessGrantResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /GetWriteAccessGrant response",
            )
        })?;
        let method = match &response.grant.redemption {
            cloud_ext::Redemption::HttpRequest(_) => {
                re_protos::cloud::v1alpha1::UploadMethod::SingleRequest
            }
            cloud_ext::Redemption::MultipartSession(_) => {
                re_protos::cloud::v1alpha1::UploadMethod::Multipart
            }
        };
        let err = if !supported_upload_methods.contains(&method) {
            ApiError::failed_precondition(
                &self.origin,
                format!(
                    "/GetWriteAccessGrant returned upload method {method:?}, but the request accepted {supported_upload_methods:?}",
                ),
            )
        } else if response.grant.signing != signing {
            ApiError::unimplemented(
                &self.origin,
                format!(
                    "/GetWriteAccessGrant returned {:?} URLs, but {signing:?} URLs were requested",
                    response.grant.signing,
                ),
            )
        } else {
            return Ok(response);
        };
        // The server has already initiated the session, and the caller never sees its token.
        if let cloud_ext::Redemption::MultipartSession(request) = response.grant.redemption
            && let Err(abort_err) = self.abort_multipart_upload(request.upload_token).await
        {
            re_log::warn!("Failed to abort rejected multipart upload: {abort_err}");
        }
        Err(err.with_trace_id(trace_id))
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_multipart_upload_part_grants(
        &mut self,
        upload_token: Bytes,
        part_numbers: Vec<u32>,
    ) -> ApiResult<Vec<cloud_ext::HttpRequest>> {
        let expected_count = part_numbers.len();
        let (response, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .get_multipart_upload_part_grants(
                    re_protos::cloud::v1alpha1::GetMultipartUploadPartGrantsRequest {
                        upload_token,
                        part_numbers,
                    },
                )
                .await
                .map_err(|err| {
                    ApiError::tonic(&self.origin, err, "/GetMultipartUploadPartGrants failed")
                })?,
        );
        if response.requests.len() != expected_count {
            return Err(ApiError::deserialization(
                &self.origin,
                trace_id,
                "multipart part grant count does not match the request",
            ));
        }
        response
            .requests
            .into_iter()
            .map(|request| {
                request.try_into().map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "failed parsing multipart part grant",
                    )
                })
            })
            .collect()
    }

    /// Publishes the uploaded object, without registering it.
    /// Retrying with the same token and receipts confirms publication, even after the lease expires.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn complete_multipart_upload(
        &mut self,
        upload_token: Bytes,
        parts: Vec<re_protos::cloud::v1alpha1::UploadedPart>,
    ) -> ApiResult<()> {
        self.inner()
            .complete_multipart_upload(re_protos::cloud::v1alpha1::CompleteMultipartUploadRequest {
                upload_token,
                parts,
            })
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/CompleteMultipartUpload failed"))?;
        Ok(())
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn abort_multipart_upload(&mut self, upload_token: Bytes) -> ApiResult<()> {
        self.inner()
            .abort_multipart_upload(re_protos::cloud::v1alpha1::AbortMultipartUploadRequest {
                upload_token,
            })
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/AbortMultipartUpload failed"))?;
        Ok(())
    }
}
