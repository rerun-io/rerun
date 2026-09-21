//! Policy-driven retries for typed RPC calls.

use futures::StreamExt as _;
use futures::future::{Either, select};
use re_protos::external::tonic_types::StatusExt as _;
use std::time::Duration;

/// Returns diagnostic context for retryable errors, or `None` to stop retrying.
type RetryReason = fn(&tonic::Status) -> Option<&str>;

struct RetryPolicy {
    base_delay: Duration,
    max_delay: Duration,
    budget: Duration,
    retry_reason: RetryReason,
}

impl RetryPolicy {
    fn backoff(&self) -> re_backoff::BackoffGenerator {
        re_backoff::BackoffGenerator::new(self.base_delay, self.max_delay)
            .expect("valid backoff bounds")
    }
}

fn default_policy() -> Vec<RetryPolicy> {
    let dataset_revision_policy = RetryPolicy {
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(1),
        budget: Duration::from_secs(10),
        retry_reason: |status: &tonic::Status| {
            if status.code() != tonic::Code::FailedPrecondition {
                return None;
            }

            match status.get_details_error_info() {
                Some(info)
                    if info.domain == re_protos::error::ERROR_DOMAIN
                        && info.reason == re_protos::error::DATASET_REVISION_BEHIND =>
                {
                    Some(status.message())
                }
                _ => None,
            }
        },
    };
    let tls_connection_policy = RetryPolicy {
        // Tonic reports transient TLS failures as `Unknown` during service readiness or as
        // `Unavailable` after dispatch.
        retry_reason: |status: &tonic::Status| {
            matches!(
                status.code(),
                tonic::Code::Unknown | tonic::Code::Unavailable
            )
            .then_some(status.message())
        },
        ..dataset_revision_policy
    };
    vec![dataset_revision_policy, tls_connection_policy]
}

/// Rebuild requests on every attempt so interceptors can refresh their metadata.
/// The budget starts at the first retryable error and bounds subsequent calls and backoff.
pub async fn retry<T, F: Future<Output = tonic::Result<T>>>(
    call: impl FnMut() -> F,
) -> tonic::Result<T> {
    retry_with_policy(default_policy(), call).await
}

async fn retry_with_policy<T, F: Future<Output = tonic::Result<T>>>(
    policies: Vec<RetryPolicy>,
    mut call: impl FnMut() -> F,
) -> tonic::Result<T> {
    let mut deadline = None;
    let mut backoff = None;
    let mut result = call().await;
    loop {
        let err = match result {
            Ok(response) => return Ok(response),
            Err(err) => err,
        };
        let Some((policy, reason)) = policies
            .iter()
            .find_map(|policy| (policy.retry_reason)(&err).map(|reason| (policy, reason)))
        else {
            return Err(err);
        };
        let mut timeout = tonic::Status::deadline_exceeded(format!("timed out {reason}"));
        if let Some(trace_id) = err.metadata().get(crate::GRPC_RESPONSE_TRACEID_HEADER) {
            timeout
                .metadata_mut()
                .insert(crate::GRPC_RESPONSE_TRACEID_HEADER, trace_id.clone());
        }
        let deadline = *deadline.get_or_insert_with(|| web_time::Instant::now() + policy.budget);
        let delay = backoff
            .get_or_insert_with(|| policy.backoff())
            .gen_next()
            .jittered()
            .min(deadline.saturating_duration_since(web_time::Instant::now()));
        tracing::debug!(%reason, ?delay, "RPC rejected, retrying after backoff");
        re_async::sleep(delay).await;
        let Some(remaining) = deadline.checked_duration_since(web_time::Instant::now()) else {
            return Err(timeout);
        };
        result = match select(
            std::pin::pin!(call()),
            std::pin::pin!(re_async::sleep(remaining)),
        )
        .await
        {
            Either::Left((result, _)) => result,
            Either::Right(_) => return Err(timeout),
        };
    }
}

/// Resolve early trailer errors before committing a stream to its caller.
pub async fn open_stream<T: Send + 'static>(
    response: tonic::Response<tonic::Streaming<T>>,
) -> tonic::Result<tonic::Response<impl futures::Stream<Item = tonic::Result<T>> + Send>> {
    let (metadata, mut stream, extensions) = response.into_parts();
    let first = stream.message().await.map_err(|mut status| {
        if crate::extract_trace_id(status.metadata()).is_none()
            && let Some(trace_id) = metadata.get(crate::GRPC_RESPONSE_TRACEID_HEADER)
        {
            status
                .metadata_mut()
                .insert(crate::GRPC_RESPONSE_TRACEID_HEADER, trace_id.clone());
        }
        status
    })?;
    // NOLINT: futures Stream::chain, not Iterator::chain
    let stream = futures::stream::iter(first.map(Ok)).chain(stream);
    Ok(tonic::Response::from_parts(metadata, stream, extensions))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use http_body::{Body as _, Frame};
    use re_log_types::EntryId;
    use re_protos::external::prost::{Message as _, bytes::Bytes};
    use re_protos::external::tonic_types::ErrorDetails;
    use std::collections::{HashMap, VecDeque};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    fn rejection(domain: &str, reason: &str) -> tonic::Status {
        tonic::Status::with_error_details(
            tonic::Code::FailedPrecondition,
            "replica behind",
            ErrorDetails::with_error_info(
                reason,
                domain,
                HashMap::from([
                    ("datasetUrl".into(), "memory://datasets/test".into()),
                    ("watermark".into(), "7".into()),
                    ("promotedRevision".into(), "6".into()),
                ]),
            ),
        )
    }

    fn guard() -> tonic::Status {
        rejection(
            re_protos::error::ERROR_DOMAIN,
            re_protos::error::DATASET_REVISION_BEHIND,
        )
    }

    #[tokio::test]
    async fn retry_uses_the_supplied_policy() {
        let policy = RetryPolicy {
            base_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
            budget: Duration::from_secs(1),
            retry_reason: |status: &tonic::Status| {
                (status.code() == tonic::Code::Unavailable).then_some(status.message())
            },
        };
        let mut attempts = 0;
        let result = retry_with_policy(vec![policy], || {
            attempts += 1;
            std::future::ready(if attempts == 1 {
                Err(tonic::Status::unavailable("busy"))
            } else {
                Ok(42)
            })
        })
        .await;
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts, 2);
    }

    #[test]
    fn transient_transport_errors_are_retryable() {
        for status in [
            tonic::Status::unknown("Service was not ready: transport error"),
            tonic::Status::unavailable("connection closed"),
        ] {
            assert_eq!(
                default_policy()
                    .iter()
                    .find_map(|policy| (policy.retry_reason)(&status)),
                Some(status.message())
            );
        }
    }

    #[tokio::test]
    async fn other_errors_are_not_retried() {
        for error in [
            tonic::Status::not_found("deleted"),
            tonic::Status::failed_precondition("unmarked"),
            rejection("other", re_protos::error::DATASET_REVISION_BEHIND),
            rejection(re_protos::error::ERROR_DOMAIN, "OTHER"),
            tonic::Status::with_details(
                tonic::Code::FailedPrecondition,
                "malformed",
                Bytes::from_static(b"invalid"),
            ),
        ] {
            let mut attempts = 0;
            let result: tonic::Result<()> = retry(|| {
                attempts += 1;
                std::future::ready(Err(error.clone()))
            })
            .await;
            assert_eq!(result.unwrap_err().code(), error.code());
            assert_eq!(attempts, 1);
        }
    }

    #[tokio::test]
    async fn deadline_bounds_backoff_and_pending_attempts() {
        for pending in [false, true] {
            let mut attempts = 0;
            let mut policies = default_policy();
            policies[0].budget = Duration::from_millis(150);
            let result: tonic::Result<()> = tokio::time::timeout(
                Duration::from_secs(2),
                retry_with_policy(policies, || {
                    attempts += 1;
                    let attempts = attempts;
                    async move {
                        if pending && attempts > 1 {
                            futures::future::pending().await
                        } else {
                            let mut err = guard();
                            err.metadata_mut().insert(
                                crate::GRPC_RESPONSE_TRACEID_HEADER,
                                format!("{attempts:032x}").parse().unwrap(),
                            );
                            Err(err)
                        }
                    }
                }),
            )
            .await
            .unwrap();
            let error = result.unwrap_err();
            assert_eq!(error.code(), tonic::Code::DeadlineExceeded);
            assert_eq!(error.message(), format!("timed out {}", guard().message()));
            assert!(attempts >= 2);
            let last_rejection = if pending { 1 } else { attempts };
            assert_eq!(
                crate::extract_trace_id(error.metadata())
                    .unwrap()
                    .to_string(),
                format!("{last_rejection:032x}"),
            );
        }
    }

    struct Frames(VecDeque<Frame<Bytes>>);

    impl http_body::Body for Frames {
        type Data = Bytes;
        type Error = tonic::Status;

        fn poll_frame(
            mut self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<tonic::Result<Frame<Bytes>>>> {
            std::task::Poll::Ready(self.0.pop_front().map(Ok))
        }
    }

    #[tokio::test]
    async fn early_stream_errors_preserve_trace_id() {
        let header_trace = "11111111111111111111111111111111";
        for trailer_trace in [None, Some("22222222222222222222222222222222")] {
            let service =
                tower::service_fn(move |_: http::Request<tonic::body::Body>| async move {
                    let mut status = tonic::Status::not_found("missing segment");
                    if let Some(trace_id) = trailer_trace {
                        status.metadata_mut().insert(
                            crate::GRPC_RESPONSE_TRACEID_HEADER,
                            trace_id.parse().unwrap(),
                        );
                    }
                    let frames = VecDeque::from([Frame::trailers(
                        status.into_http::<()>().into_parts().0.headers,
                    )]);
                    let mut response = http::Response::new(tonic::body::Body::new(Frames(frames)));
                    response.headers_mut().insert(
                        "content-type",
                        http::HeaderValue::from_static("application/grpc"),
                    );
                    response.headers_mut().insert(
                        crate::GRPC_RESPONSE_TRACEID_HEADER,
                        header_trace.parse().unwrap(),
                    );
                    tonic::Result::Ok(response)
                });
            let client = crate::RedapClient::new(
                re_uri::Origin::test(),
                crate::grpc::boxed_redap_grpc_client(service),
                None,
            );
            let Err(err) = client
                .query_dataset_raw(crate::SegmentQueryParams {
                    dataset_id: EntryId::new(),
                    segment_id: "segment".into(),
                    include_static_data: true,
                    include_temporal_data: true,
                    generate_direct_urls: false,
                    unsigned_direct_urls: false,
                    query: None,
                })
                .await
            else {
                panic!("expected an early stream error");
            };
            assert_eq!(err.kind, crate::ApiErrorKind::NotFound);
            let expected_trace = trailer_trace.unwrap_or(header_trace);
            assert!(
                err.to_string()
                    .contains(&format!("trace-id: {expected_trace}"))
            );
        }
    }

    #[tokio::test]
    async fn query_retries_early_errors_and_restamps_but_never_restarts_after_data() {
        for (trailers, committed, floored) in [
            (false, false, true),
            (true, false, true),
            (true, true, true),
            (false, false, false),
        ] {
            let id = EntryId::new();
            if floored {
                crate::dataset_revisions().observe(id, 7);
            }
            let attempts = Arc::new(AtomicUsize::new(0));
            let recorded = Arc::new(re_mutex::Mutex::new(None));
            let service = tower::service_fn({
                let attempts = attempts.clone();
                move |mut request: http::Request<tonic::body::Body>| {
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                    let recorded = recorded.clone();
                    async move {
                        let watermark = request
                            .headers()
                            .get("x-rerun-dataset-revision")
                            .map(|v| v.to_str().unwrap());
                        assert_eq!(
                            watermark,
                            if !floored {
                                None
                            } else if attempt == 0 {
                                Some("7")
                            } else {
                                Some("9")
                            }
                        );
                        assert_eq!(
                            request.uri().path(),
                            "/rerun.cloud.v1alpha1.RerunCloudService/QueryDataset"
                        );
                        let mut bytes = Vec::new();
                        while let Some(frame) = futures::future::poll_fn(|cx| {
                            std::pin::Pin::new(request.body_mut()).poll_frame(cx)
                        })
                        .await
                        {
                            if let Ok(data) = frame.unwrap().into_data() {
                                bytes.extend_from_slice(&data);
                            }
                        }
                        {
                            let mut recorded = recorded.lock();
                            if let Some(first) = recorded.as_ref() {
                                assert_eq!(first, &bytes);
                            } else {
                                *recorded = Some(bytes);
                            }
                        }
                        let reject = floored && attempt == 0;
                        let status = if reject {
                            guard()
                        } else {
                            tonic::Status::ok("")
                        };
                        if reject {
                            crate::dataset_revisions().observe(id, 9);
                        }
                        if reject && !trailers {
                            return tonic::Result::Ok(status.into_http());
                        }
                        let mut frames = VecDeque::new();
                        if !reject || committed {
                            let message =
                                re_protos::cloud::v1alpha1::QueryDatasetResponse::default()
                                    .encode_to_vec();
                            let mut data = vec![0];
                            data.extend_from_slice(&(message.len() as u32).to_be_bytes());
                            data.extend_from_slice(&message);
                            frames.push_back(Frame::data(Bytes::from(data)));
                        }
                        frames.push_back(Frame::trailers(
                            status.into_http::<()>().into_parts().0.headers,
                        ));
                        let mut response =
                            http::Response::new(tonic::body::Body::new(Frames(frames)));
                        response.headers_mut().insert(
                            "content-type",
                            http::HeaderValue::from_static("application/grpc"),
                        );
                        Ok(response)
                    }
                }
            });
            let client = crate::RedapClient::new(
                re_uri::Origin::test(),
                crate::grpc::boxed_redap_grpc_client(service),
                None,
            );
            let mut stream = client
                .query_dataset_raw(crate::SegmentQueryParams {
                    dataset_id: id,
                    segment_id: "segment".into(),
                    include_static_data: true,
                    include_temporal_data: true,
                    generate_direct_urls: false,
                    unsigned_direct_urls: false,
                    query: None,
                })
                .await
                .unwrap();
            assert!(stream.next().await.unwrap().is_ok());
            if committed {
                assert!(stream.next().await.unwrap().is_err());
            } else {
                assert!(stream.next().await.is_none());
            }
            assert_eq!(
                attempts.load(Ordering::SeqCst),
                if floored && !committed { 2 } else { 1 }
            );
        }
    }
}
