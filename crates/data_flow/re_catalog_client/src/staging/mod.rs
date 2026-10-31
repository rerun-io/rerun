//! Stage objects in catalog storage.

mod multipart;

use re_async::AsyncReadAt;
use re_protos::cloud::v1alpha1::UploadMethod;
use re_protos::cloud::v1alpha1::ext::{
    GetWriteAccessGrantRequest, GetWriteAccessGrantResponse, ObjectKey, Redemption, UrlSigning,
};
use re_redap_client::ApiError;
use re_span::Span;

use crate::CatalogClient;

/// Upload method options, primarily for testing.
/// By default, the server selects between single-request and multipart uploads.
#[derive(Clone, Debug)]
pub struct StageOpt {
    upload_methods: Vec<UploadMethod>,
}

impl StageOpt {
    /// Requires a single-request upload.
    pub fn single() -> Self {
        Self {
            upload_methods: vec![UploadMethod::SingleRequest],
        }
    }

    /// Requires a multipart upload, even for objects small enough for a single request.
    pub fn multipart() -> Self {
        Self {
            upload_methods: vec![UploadMethod::Multipart],
        }
    }
}

impl Default for StageOpt {
    fn default() -> Self {
        Self {
            upload_methods: vec![UploadMethod::SingleRequest, UploadMethod::Multipart],
        }
    }
}

/// An error staging an object in a catalog's storage.
#[derive(Debug, thiserror::Error)]
pub enum StagingError {
    #[error(transparent)]
    Api(#[from] ApiError),

    #[error("the write access grant expired at {expires_at}")]
    Expired { expires_at: jiff::Timestamp },

    #[error("invalid multipart upload grant: {0}")]
    InvalidMultipart(String),

    #[error("invalid upload request in the grant: {0}")]
    InvalidUploadRequest(String),

    #[error(
        "multipart part {part_number} response has no ETag; check the bucket's CORS ExposeHeaders"
    )]
    MissingEtag { part_number: u32 },

    #[error("failed to read the source: {0}")]
    Read(#[from] std::io::Error),

    #[error(
        "the upload was rejected: HTTP {status} {status_text}{}",
        code.as_ref().map(|code| format!(" ({code})")).unwrap_or_default()
    )]
    Rejected {
        status: u16,
        status_text: String,

        /// Storage-service error code from the response body, e.g. S3's `ExpiredToken`.
        code: Option<String>,
    },

    #[error("failed to send the source: {0}")]
    Request(String),
}

impl CatalogClient {
    /// Stages `source` in the catalog's storage under `key` and returns its credential-free URL
    /// after the upload succeeds.
    // TODO(RR-6055): Accept object_store_config in the staging options.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn stage(
        &self,
        key: ObjectKey,
        source: impl AsyncReadAt,
    ) -> Result<url::Url, StagingError> {
        // Keep the upload state machine out of callers' future layouts.
        Box::pin(self.stage_opt(key, source, StageOpt::default())).await
    }

    /// Stages an object using only the accepted upload methods.
    /// An unsupported method results in an error from the server.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn stage_opt(
        &self,
        key: ObjectKey,
        source: impl AsyncReadAt,
        opt: StageOpt,
    ) -> Result<url::Url, StagingError> {
        let StageOpt { upload_methods } = opt;
        // TODO(RR-5890): Accept sequential sources without requiring the total size up front.
        let source_size_bytes = source.size().await?;
        let request = GetWriteAccessGrantRequest {
            key,
            // TODO(RR-6055): Forward the selected object_store_config in the grant request.
            location: None,
            size_bytes: Some(source_size_bytes),
            supported_upload_methods: upload_methods,
            signing: UrlSigning::Signed,
        };
        let GetWriteAccessGrantResponse { storage_url, grant } =
            self.client().await?.get_write_access_grant(request).await?;

        let expires_at = grant.expires_at;
        match grant.redemption {
            Redemption::HttpRequest(request) => {
                check_expiry(expires_at)?;
                // TODO(RR-5715): Stream the HTTP request body instead of buffering the whole source.
                let body = source
                    .read_exact_at(Span {
                        start: 0,
                        len: source_size_bytes,
                    })
                    .await?;
                check_expiry(expires_at)?;
                self.send_upload(request, Vec::from(body)).await?;
            }
            Redemption::MultipartRequest(request) => {
                self.stage_multipart(request, expires_at, &source, source_size_bytes)
                    .await?;
            }
        }
        Ok(storage_url)
    }

    async fn send_upload(
        &self,
        http_request: re_protos::cloud::v1alpha1::ext::HttpRequest,
        body: Vec<u8>,
    ) -> Result<ehttp::Response, StagingError> {
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
                    .map_err(|err| StagingError::InvalidUploadRequest(err.to_string()))?;
                (method, url, headers)
            }
        };

        let mut request = ehttp::Request::post(url.as_str(), body);
        request.method =
            ehttp::Method::parse(method.as_str()).map_err(StagingError::InvalidUploadRequest)?;
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
        let response = response
            .map_err(|err| StagingError::Request(err.replace(url.as_str(), "<upload URL>")))?;
        if !response.ok {
            let code = response.text().and_then(error_code).map(str::to_owned);
            return Err(StagingError::Rejected {
                status: response.status,
                status_text: response.status_text,
                code,
            });
        }

        Ok(response)
    }
}

/// Extracts `<Code>` from an S3-style XML error body.
fn error_code(body: &str) -> Option<&str> {
    let (_, rest) = body.split_once("<Code>")?;
    let (code, _) = rest.split_once("</Code>")?;
    Some(code.trim()).filter(|code| !code.is_empty())
}

fn check_expiry(expires_at: jiff::Timestamp) -> Result<(), StagingError> {
    if jiff::Timestamp::now() >= expires_at {
        Err(StagingError::Expired { expires_at })
    } else {
        Ok(())
    }
}
