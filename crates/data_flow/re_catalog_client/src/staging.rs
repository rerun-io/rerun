//! Stage objects in catalog storage.

use re_async::AsyncReadAt;
use re_protos::cloud::v1alpha1::UploadMethod;
use re_protos::cloud::v1alpha1::ext::{
    GetWriteAccessGrantRequest, GetWriteAccessGrantResponse, ObjectKey, Redemption, UrlSigning,
};
use re_span::Span;

use crate::CatalogClient;
use re_redap_client::ApiError;

/// An error staging an object in a catalog's storage.
#[derive(Debug, thiserror::Error)]
pub enum StagingError {
    #[error(transparent)]
    Api(#[from] ApiError),

    #[error("the write access grant expired at {expires_at}")]
    Expired { expires_at: jiff::Timestamp },

    #[error("failed to read the source: {0}")]
    Read(#[from] std::io::Error),

    #[error("failed to send the source: {0}")]
    Request(String),

    #[error("the upload was rejected: HTTP {status} {status_text}")]
    Rejected { status: u16, status_text: String },

    #[error("unsupported upload method: {0:?}")]
    UnsupportedUploadMethod(UploadMethod),
}

impl CatalogClient {
    /// Stages `source` in the catalog's storage under `key` and returns its credential-free URL
    /// after the upload succeeds.
    // TODO(RR-5715): Stream uploads with bounded memory and multipart support.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn stage(
        &self,
        key: ObjectKey,
        source: impl AsyncReadAt,
    ) -> Result<url::Url, StagingError> {
        let size = source.size().await?;
        let request = GetWriteAccessGrantRequest {
            key,
            location: None,
            size_bytes: Some(size),
            supported_upload_methods: vec![UploadMethod::SingleRequest],
            signing: UrlSigning::Signed,
        };
        let GetWriteAccessGrantResponse { storage_url, grant } =
            self.client().await?.get_write_access_grant(request).await?;

        let expires_at = grant.expires_at;
        let http_request = match grant.redemption {
            Redemption::HttpRequest(request) => request,
            Redemption::MultipartRequest(_) => {
                return Err(StagingError::UnsupportedUploadMethod(
                    UploadMethod::Multipart,
                ));
            }
        };
        if jiff::Timestamp::now() >= expires_at {
            return Err(StagingError::Expired { expires_at });
        }

        let body = source
            .read_exact_at(Span {
                start: 0,
                len: size,
            })
            .await?;
        let (method, url, headers) = match http_request {
            re_protos::cloud::v1alpha1::ext::HttpRequest::External {
                method,
                url,
                headers,
            } => (method, url, headers),
            re_protos::cloud::v1alpha1::ext::HttpRequest::SameOrigin {
                method,
                path_and_query,
                headers,
            } => {
                let url = url::Url::parse(&self.origin().as_url())
                    .and_then(|base_url| base_url.join(path_and_query.as_str()))
                    .map_err(|err| StagingError::Request(err.to_string()))?;
                (method, url, headers)
            }
        };

        // The whole object is in memory here, so take the buffer rather than copying it again:
        // `Bytes` that uniquely owns its allocation converts back into a `Vec` for free.
        let mut request = ehttp::Request::post(url.as_str(), Vec::from(body));
        request.method = ehttp::Method::parse(method.as_str()).map_err(StagingError::Request)?;
        request.headers = ehttp::Headers {
            headers: headers
                .iter()
                .map(|(name, value)| {
                    (
                        name.to_string(),
                        String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    )
                })
                .collect(),
        };

        re_crypto::init();
        cfg_select! {
            target_family = "wasm" => {
                let response = re_async::spawn_local_with_result(ehttp::fetch_async(request))
                    .await
                    .unwrap_or_else(|_| Err("HTTP request was canceled".to_owned()));
            }
            _ => {
                let response = ehttp::fetch_async(request).await;
            }
        }
        let response = response.map_err(StagingError::Request)?;
        if !response.ok {
            return Err(StagingError::Rejected {
                status: response.status,
                status_text: response.status_text,
            });
        }

        Ok(storage_url)
    }
}
