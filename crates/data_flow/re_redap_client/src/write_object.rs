use re_async::AsyncReadAt;
use re_protos::cloud::v1alpha1::ext::{GetWriteAccessGrantResponse, ObjectKey, Redemption};
use re_span::Span;

use crate::{ApiError, ConnectionHandle};

/// An error writing an object to a catalog's storage.
#[derive(Debug, thiserror::Error)]
pub enum WriteObjectError {
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
}

impl ConnectionHandle {
    /// Writes `source` to the catalog's storage under `key` and returns the credential-free URL
    /// that names the object.
    ///
    /// Writing does not register the object: pass the returned URL to
    /// [`Self::register_with_dataset`], which is a separate operation and may happen much later.
    ///
    /// The source must return stable contents for the duration of the upload.
    ///
    /// TODO(RR-5715): Stream the request body instead of buffering the whole source so large
    /// uploads do not need equivalent memory.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn write_object(
        &self,
        key: ObjectKey,
        source: impl AsyncReadAt,
    ) -> Result<url::Url, WriteObjectError> {
        let size = source.size().await?;
        let GetWriteAccessGrantResponse { storage_url, grant } = self
            .client()
            .await?
            .get_write_access_grant(key, size)
            .await?;

        if jiff::Timestamp::now() >= grant.expires_at {
            return Err(WriteObjectError::Expired {
                expires_at: grant.expires_at,
            });
        }

        let body = source
            .read_exact_at(Span {
                start: 0,
                len: size,
            })
            .await?;

        let Redemption::HttpRequest(http_request) = grant.redemption;
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
                    .map_err(|err| WriteObjectError::Request(err.to_string()))?;
                (method, url, headers)
            }
        };

        // The whole object is in memory here, so take the buffer rather than copying it again:
        // `Bytes` that uniquely owns its allocation converts back into a `Vec` for free.
        let mut request = ehttp::Request::post(url.as_str(), Vec::from(body));
        request.method =
            ehttp::Method::parse(method.as_str()).map_err(WriteObjectError::Request)?;
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
        let response = response.map_err(WriteObjectError::Request)?;
        if !response.ok {
            return Err(WriteObjectError::Rejected {
                status: response.status,
                status_text: response.status_text,
            });
        }

        Ok(storage_url)
    }
}
