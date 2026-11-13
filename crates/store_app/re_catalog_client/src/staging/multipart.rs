use std::num::{NonZeroU32, NonZeroU64};
use std::time::Duration;

use re_async::{AsyncReadAt, AsyncRuntimeHandle};
use re_protos::cloud::v1alpha1::UploadedPart;
use re_protos::cloud::v1alpha1::ext::MultipartSession;
use re_protos::external::prost::bytes::Bytes;
use re_redap_client::ApiResult;
use re_span::Span;

use super::{StagingError, check_expiry};
use crate::CatalogClient;

/// Per-part or completion attempt budget, including the initial attempt and failed grant refreshes.
const MAX_ATTEMPTS: u32 = 4;

/// Client memory budget, not a provider limit: a retained retry body plus its HTTP copy
/// use at most 128 MiB per upload, excluding source memory and transport buffers.
const MAX_PART_SIZE_BYTES: u64 = 64 * 1024 * 1024;

/// Compatibility with grants from servers that do not advertise a part-count limit.
// TODO(RR-6296): Remove this fallback once the oldest supported Hub version guarantees
// `max_part_count` in every multipart grant; reject missing limits at that point.
const LEGACY_MAX_PART_COUNT: u32 = 10_000;

/// Retry delays for transient failures, reset for each part and for completion.
const RETRY_BACKOFF: re_backoff::BackoffGenerator =
    re_backoff::BackoffGenerator::new_const(Duration::from_millis(250), Duration::from_secs(1));

impl CatalogClient {
    pub(super) async fn stage_multipart(
        &self,
        request: MultipartSession,
        expires_at: jiff::Timestamp,
        source: &impl AsyncReadAt,
        source_size_bytes: u64,
    ) -> Result<(), StagingError> {
        let upload_token = request.upload_token.clone();
        let mut abort = AbortUploadGuard::new(self, upload_token.clone());

        let upload = self.upload_multipart_parts(request, expires_at, source, source_size_bytes);
        let remaining = expires_at.duration_since(jiff::Timestamp::now());
        let parts = re_async::timeout(remaining.try_into().unwrap_or(Duration::ZERO), upload)
            .await
            .unwrap_or_else(|_| Err(StagingError::Expired { expires_at }));

        // Completion runs without the deadline: the server enforces the lease when it receives
        // the request, and cancelling an in-flight completion cannot stop S3 from publishing.
        let result = async {
            let parts = parts?;
            check_expiry(expires_at)?;
            retry_completion(|| {
                let upload_token = upload_token.clone();
                let parts = parts.clone();
                async move {
                    self.client()
                        .await?
                        .complete_multipart_upload(upload_token, parts)
                        .await
                }
            })
            .await?;
            Ok(())
        }
        .await;
        if result.is_ok() {
            abort.token = None;
        } else {
            abort.abort().await;
        }
        result
    }

    async fn upload_multipart_parts(
        &self,
        request: MultipartSession,
        expires_at: jiff::Timestamp,
        source: &impl AsyncReadAt,
        source_size_bytes: u64,
    ) -> Result<Vec<UploadedPart>, StagingError> {
        check_expiry(expires_at)?;
        let MultipartSession {
            upload_token,
            part_size_bytes,
            part_requests,
            max_part_count,
        } = request;
        let ranges = part_ranges(source_size_bytes, part_size_bytes, max_part_count)?;
        let part_count = ranges.len() as u32;
        let mut requests = part_requests.into_iter();
        let mut parts = Vec::new();

        // TODO(grtlr): Upload parts with bounded concurrency and independent grant refreshes per part.
        for PartRange { part_number, span } in ranges {
            check_expiry(expires_at)?;
            // Keep these bytes until acknowledged; rereading a mutable source could change a retry.
            let body = source.read_exact_at(span).await?;
            let mut request = requests.next();
            let mut backoff = RETRY_BACKOFF;

            for attempt in 1..=MAX_ATTEMPTS {
                check_expiry(expires_at)?;
                let result = async {
                    if request.is_none() {
                        requests = self
                            .client()
                            .await?
                            .get_multipart_upload_part_grants(
                                upload_token.clone(),
                                part_grant_batch(part_number, part_count),
                            )
                            .await?
                            .into_iter();
                        request = requests.next();
                    }
                    let request = request.clone().ok_or_else(|| {
                        StagingError::InvalidMultipart("missing part grant".to_owned())
                    })?;
                    self.send_upload(request, body.to_vec()).await
                }
                .await;
                match result {
                    Ok(response) => {
                        parts.push(uploaded_part(part_number, &response.headers)?);
                        break;
                    }
                    Err(err) if should_retry(&err, attempt) => {
                        // Credentials can expire before the lease; discard other grants too.
                        request = None;
                        requests = Vec::new().into_iter();
                        backoff.gen_next().sleep().await;
                    }
                    Err(err) => return Err(err),
                }
            }
        }
        Ok(parts)
    }
}

/// Every attempt must use the same token and receipts.
/// Confirmation may outlive the lease; the server enforces publication expiry.
async fn retry_completion<F: Future<Output = ApiResult<()>>>(
    mut complete: impl FnMut() -> F,
) -> ApiResult<()> {
    let mut backoff = RETRY_BACKOFF;
    let mut attempt = 1;
    loop {
        match complete().await {
            Err(err) if err.kind.is_retryable() && attempt < MAX_ATTEMPTS => {
                re_log::debug!("Multipart completion attempt {attempt} failed: {err}");
                backoff.gen_next().sleep().await;
                attempt += 1;
            }
            result => return result,
        }
    }
}

struct PartRange {
    part_number: u32,
    span: Span<u64>,
}

fn part_ranges(
    source_size_bytes: u64,
    part_size_bytes: NonZeroU64,
    max_part_count: Option<NonZeroU32>,
) -> Result<impl ExactSizeIterator<Item = PartRange>, StagingError> {
    let part_size_bytes = part_size_bytes.get();
    if part_size_bytes > MAX_PART_SIZE_BYTES {
        return Err(StagingError::InvalidMultipart(format!(
            "part size exceeds {}",
            re_format::format_bytes(MAX_PART_SIZE_BYTES as f64),
        )));
    }
    let part_count = source_size_bytes.div_ceil(part_size_bytes).max(1);
    let max_part_count = max_part_count.map_or(LEGACY_MAX_PART_COUNT, NonZeroU32::get);
    if part_count > u64::from(max_part_count) {
        return Err(StagingError::MultipartPartLimit {
            size_bytes: source_size_bytes,
            required_parts: part_count,
            part_size_bytes,
            max_part_count,
        });
    }
    // The count fits in u32 after validation against max_part_count.
    let part_count = part_count as u32;
    Ok((0..part_count).map(move |index| {
        let start = u64::from(index) * part_size_bytes;
        PartRange {
            part_number: index + 1,
            span: Span {
                start,
                len: (source_size_bytes - start).min(part_size_bytes),
            },
        }
    }))
}

fn part_grant_batch(part_number: u32, part_count: u32) -> Vec<u32> {
    (part_number..=part_count.min(part_number.saturating_add(15))).collect()
}

fn uploaded_part(part_number: u32, headers: &ehttp::Headers) -> Result<UploadedPart, StagingError> {
    let etag = headers.get("etag").unwrap_or_default();
    // Browsers hide the header unless the bucket's CORS rule exposes it. That is far more likely
    // than a provider that returns no ETag, so report it before completion fails less clearly.
    if cfg!(target_family = "wasm") && etag.is_empty() {
        return Err(StagingError::MissingEtag { part_number });
    }
    Ok(UploadedPart {
        part_number,
        etag: etag.to_owned(),
    })
}

fn should_retry(err: &StagingError, attempts: u32) -> bool {
    if attempts >= MAX_ATTEMPTS {
        return false;
    }
    match err {
        StagingError::Api(err) => err.kind.is_retryable(),
        StagingError::Rejected { status, code, .. } => {
            matches!(status, 401 | 403 | 408 | 429 | 500..=599)
                // S3 and GCS report these with HTTP 400.
                || matches!(
                    code.as_deref(),
                    Some("ExpiredToken" | "RequestTimeout" | "TokenRefreshRequired")
                )
        }
        StagingError::Request(_) => true,
        StagingError::Expired { .. }
        | StagingError::InvalidMultipart(_)
        | StagingError::InvalidUploadRequest(_)
        | StagingError::MissingEtag { .. }
        | StagingError::MultipartPartLimit { .. }
        | StagingError::Read(_) => false,
    }
}

struct AbortUploadGuard<'a> {
    client: &'a CatalogClient,
    token: Option<Bytes>,
}

impl<'a> AbortUploadGuard<'a> {
    fn new(client: &'a CatalogClient, token: Bytes) -> Self {
        Self {
            client,
            token: Some(token),
        }
    }

    async fn abort(&mut self) {
        let Some(token) = self.token.clone() else {
            return;
        };
        abort_upload(self.client, token).await;
        self.token = None;
    }
}

impl Drop for AbortUploadGuard<'_> {
    fn drop(&mut self) {
        let Self { client, token } = self;
        let Some(token) = token.take() else {
            return;
        };
        let cleanup_client = CatalogClient::clone(client);
        // The client's runtime may no longer be driven when another runtime cancels the upload.
        let current_runtime = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen();
        let runtime = current_runtime.as_ref().unwrap_or(&client.runtime);
        // In-flight HTTP requests can outlive cancellation. Bucket lifecycle rules must
        // also remove parts that arrive after this best-effort abort.
        runtime.spawn_future(async move {
            abort_upload(&cleanup_client, token).await;
        });
    }
}

async fn abort_upload(client: &CatalogClient, token: Bytes) {
    let request = async { client.client().await?.abort_multipart_upload(token).await };
    match re_async::timeout(Duration::from_secs(5), request).await {
        Ok(Ok(())) => {}
        Ok(Err(err)) => re_log::warn!("Failed to abort multipart upload: {err}"),
        Err(_) => re_log::warn!("Multipart abort timed out"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::staging::error_code;

    #[test]
    fn part_ranges_cover_empty_exact_and_short_final_parts() {
        for (size, expected) in [
            (0, vec![(0, 0)]),
            (1, vec![(0, 1)]),
            (6, vec![(0, 3), (3, 3)]),
            (7, vec![(0, 3), (3, 3), (6, 1)]),
        ] {
            let ranges = part_ranges(size, NonZeroU64::new(3).expect("positive part size"), None)
                .expect("source should fit within the legacy limit");
            assert_eq!(ranges.len(), expected.len());
            for (PartRange { part_number, span }, (index, (start, len))) in
                std::iter::zip(ranges, expected.into_iter().enumerate())
            {
                assert_eq!(part_number, index as u32 + 1);
                assert_eq!(span, Span { start, len });
            }
        }
    }

    #[test]
    fn part_ranges_enforce_memory_and_legacy_count_limits() {
        for (size, part_size, valid) in [
            (0, MAX_PART_SIZE_BYTES, true),
            (0, MAX_PART_SIZE_BYTES + 1, false),
            (MAX_PART_SIZE_BYTES, MAX_PART_SIZE_BYTES, true),
            (10_000, 1, true),
            (10_001, 1, false),
            (10_000 * MAX_PART_SIZE_BYTES, MAX_PART_SIZE_BYTES, true),
            (10_000 * MAX_PART_SIZE_BYTES + 1, MAX_PART_SIZE_BYTES, false),
            (u64::from(u32::MAX) + 1, 1, false),
            (u64::MAX, MAX_PART_SIZE_BYTES, false),
        ] {
            let result = part_ranges(
                size,
                NonZeroU64::new(part_size).expect("positive part size"),
                None,
            );
            assert_eq!(result.is_ok(), valid, "size={size}, part_size={part_size}");
            if let Err(err) = result {
                if part_size > MAX_PART_SIZE_BYTES {
                    assert!(matches!(err, StagingError::InvalidMultipart(_)));
                } else {
                    assert!(matches!(
                        err,
                        StagingError::MultipartPartLimit {
                            max_part_count: LEGACY_MAX_PART_COUNT,
                            ..
                        }
                    ));
                }
                assert!(!should_retry(&err, 1));
            }
        }
    }

    #[tokio::test]
    async fn rejects_part_limit_before_reading_source_bytes() {
        let runtime = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
            .expect("test should have a Tokio runtime");
        let connection = re_redap_client::ConnectionRegistry::new_without_stored_credentials()
            .connection_handle(re_uri::Origin::test());
        let client = CatalogClient::new(connection, runtime);
        let session = MultipartSession {
            upload_token: Bytes::from_static(b"token"),
            part_size_bytes: NonZeroU64::new(3).expect("positive part size"),
            part_requests: vec![],
            max_part_count: NonZeroU32::new(2),
        };
        let err = client
            .upload_multipart_parts(session, jiff::Timestamp::MAX, &Bytes::new(), 7)
            .await
            .expect_err("source should exceed the part-count limit");
        assert!(matches!(err, StagingError::MultipartPartLimit { .. }));
    }

    #[test]
    fn part_ranges_enforce_advertised_count_before_narrowing() {
        let part_size = NonZeroU64::new(3).expect("positive part size");
        assert_eq!(
            part_ranges(6, part_size, NonZeroU32::new(2))
                .expect("source should fit exactly at the part-count limit")
                .len(),
            2
        );
        let Err(err) = part_ranges(7, part_size, NonZeroU32::new(2)) else {
            panic!("expected part-count limit error");
        };
        assert!(matches!(
            err,
            StagingError::MultipartPartLimit {
                size_bytes: 7,
                required_parts: 3,
                part_size_bytes: 3,
                max_part_count: 2,
            }
        ));
        assert!(!should_retry(&err, 1));

        let max_size = u64::from(u32::MAX) * MAX_PART_SIZE_BYTES;
        for (size, part_size, expected_count) in [
            (max_size, MAX_PART_SIZE_BYTES, Some(u32::MAX as usize)),
            (max_size + 1, MAX_PART_SIZE_BYTES, None),
            (u64::MAX, 1, None),
        ] {
            let result = part_ranges(
                size,
                NonZeroU64::new(part_size).expect("positive part size"),
                NonZeroU32::new(u32::MAX),
            );
            assert_eq!(result.map(|ranges| ranges.len()).ok(), expected_count);
        }
    }

    #[test]
    fn grant_batches_start_at_the_current_part_and_stop_at_the_limit() {
        assert_eq!(part_grant_batch(1, 1), vec![1]);
        assert_eq!(part_grant_batch(2, 4), vec![2, 3, 4]);
        assert_eq!(part_grant_batch(1, 17), (1..=16).collect::<Vec<_>>());
        assert_eq!(part_grant_batch(17, 33), (17..=32).collect::<Vec<_>>());
        assert_eq!(
            part_grant_batch(u32::MAX - 1, u32::MAX),
            vec![u32::MAX - 1, u32::MAX]
        );
        assert_eq!(
            part_grant_batch(9_995, 10_000),
            (9_995..=10_000).collect::<Vec<_>>()
        );
    }

    #[test]
    fn uploaded_part_preserves_the_etag_verbatim_and_accepts_none() {
        for (headers, etag) in [
            (
                ehttp::Headers::new(&[("ETag", "\"opaque-etag\"")]),
                "\"opaque-etag\"",
            ),
            (ehttp::Headers::default(), ""),
            (ehttp::Headers::new(&[("etag", "")]), ""),
        ] {
            let part = uploaded_part(7, &headers).unwrap();
            assert_eq!(part.part_number, 7);
            assert_eq!(part.etag, etag);
        }
    }

    #[test]
    fn retries_transient_errors_with_attempts_remaining() {
        let rejected = |status, code: Option<&str>| StagingError::Rejected {
            status,
            status_text: String::new(),
            code: code.map(str::to_owned),
        };
        let unavailable = rejected(503, None);
        assert!(should_retry(&unavailable, MAX_ATTEMPTS - 1));
        assert!(!should_retry(&unavailable, MAX_ATTEMPTS));
        assert!(should_retry(&rejected(400, Some("ExpiredToken")), 1));
        assert!(!should_retry(&rejected(400, None), 1));
        assert!(should_retry(
            &StagingError::Request("connection reset".to_owned()),
            1,
        ));
        assert!(!should_retry(
            &StagingError::Read(std::io::Error::other("read failed")),
            1,
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn completion_retries_transient_errors() {
        let mut attempts = 0;
        retry_completion(|| {
            attempts += 1;
            std::future::ready(if attempts == 1 {
                Err(re_redap_client::ApiError::connection(
                    &re_uri::Origin::test(),
                    "lost completion response",
                ))
            } else {
                Ok(())
            })
        })
        .await
        .unwrap();
        assert_eq!(attempts, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn completion_bounds_retries_and_preserves_terminal_errors() {
        use re_redap_client::{ApiError, ApiErrorKind};

        for (kind, expected_attempts) in [
            (ApiErrorKind::Connection, MAX_ATTEMPTS),
            (ApiErrorKind::AlreadyExists, 1),
        ] {
            let mut attempts = 0;
            let err = retry_completion(|| {
                attempts += 1;
                let mut err = ApiError::connection(&re_uri::Origin::test(), "completion failed");
                err.kind = kind;
                std::future::ready(Err(err))
            })
            .await
            .unwrap_err();
            assert_eq!(err.kind, kind);
            assert_eq!(attempts, expected_attempts, "{kind}");
        }
    }

    #[test]
    fn rejects_expired_grants() {
        assert!(matches!(
            check_expiry(jiff::Timestamp::UNIX_EPOCH),
            Err(StagingError::Expired { .. })
        ));
        assert!(check_expiry(jiff::Timestamp::MAX).is_ok());
    }

    #[test]
    fn error_code_reads_s3_style_xml_bodies() {
        assert_eq!(
            error_code(
                "<?xml version=\"1.0\"?><Error><Code>ExpiredToken</Code><Message>…</Message></Error>"
            ),
            Some("ExpiredToken")
        );
        for body in [
            "",
            "Bad Request",
            "<Error><Code></Code></Error>",
            "<Code>Unclosed",
        ] {
            assert_eq!(error_code(body), None, "{body}");
        }
    }
}
