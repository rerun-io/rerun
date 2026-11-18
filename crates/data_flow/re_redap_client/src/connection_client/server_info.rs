use re_protos::cloud::v1alpha1::VersionRequest;
use re_protos::cloud::v1alpha1::ext::{self as cloud_ext, VersionResponse};
use re_protos::external::prost::bytes::Bytes;
use tokio_stream::StreamExt as _;
use tonic::codegen::{Body, StdError};

use super::RedapClient;
use crate::{ApiError, ApiResult};

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    /// Uses the `/Version` endpoint for testing roundtrip time.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn ping(&mut self) -> ApiResult<()> {
        self.inner()
            .version(VersionRequest {})
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/Version"))
            .map(|_| ())
    }

    /// Returns version and deployment information from the server.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn version_info(&mut self) -> ApiResult<VersionResponse> {
        let response = self
            .inner()
            .version(VersionRequest {})
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/Version"))?
            .into_inner();
        Ok(response.into())
    }

    /// Checks whether the server advertises a given feature flag in its
    /// `VersionResponse.features` list.
    ///
    /// Returns `Ok(false)` for both "feature genuinely not supported" and
    /// "old server returned an empty `features` list" — callers should
    /// treat these the same and fall back to the pre-feature path. That
    /// invariant is what lets the empty-list-from-old-server case be
    /// indistinguishable from a feature opt-out without breaking callers.
    ///
    /// The `features` list is fetched once via `/Version` on the first
    /// invocation and cached on the client (shared across clones via
    /// `Arc<OnceCell<_>>`). Subsequent calls do not hit the wire.
    pub async fn supports_feature(&mut self, feature: &str) -> ApiResult<bool> {
        // `OnceCell::get_or_try_init` single-flights the fetch: concurrent
        // first calls produce a single Version RPC; later calls return the
        // cached list directly.
        let features_cache;
        let features = if let Some(features) = self.features.get() {
            features
        } else {
            // We can't pass `&mut self` into the async closure (the `OnceCell`
            // borrow on `self.features` is immutable, but `version_info` needs
            // `&mut self`), so we clone the `Arc<OnceCell<_>>` and call
            // `version_info` outside the closure when the cell is empty.
            features_cache = self.features.clone();
            features_cache
                .get_or_try_init(|| async {
                    let info = self.version_info().await?;
                    Ok(info.features)
                })
                .await?
        };
        Ok(features.iter().any(|f| f == feature))
    }

    /// Calls the `/WhoAmI` endpoint to verify authentication and retrieve the user's identity
    /// and permissions.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn who_am_i(&mut self) -> ApiResult<re_protos::cloud::v1alpha1::WhoAmIResponse> {
        self.inner()
            .who_am_i(re_protos::cloud::v1alpha1::WhoAmIRequest {})
            .await
            .map(|resp| resp.into_inner())
            .map_err(|err| ApiError::tonic(&self.origin, err, "/WhoAmI"))
    }

    /// Estimate the round-trip time to the server.
    ///
    /// Performs `num_pings` calls to `/DoBandwidthTest` with `num_bytes = 1` and returns the
    /// minimum elapsed time. Using the minimum (rather than the mean) helps reject latency spikes
    /// from scheduling jitter, or transient network congestion.
    pub async fn rtt(&mut self, num_pings: usize) -> ApiResult<std::time::Duration> {
        if num_pings == 0 {
            return Err(ApiError::invalid_arguments(
                &self.origin,
                "rtt requires at least one ping",
            ));
        }

        let mut best = std::time::Duration::MAX;
        for _ in 0..num_pings {
            let start = web_time::Instant::now();
            let mut stream = self
                .inner()
                .do_bandwidth_test(re_protos::cloud::v1alpha1::DoBandwidthTestRequest {
                    num_bytes: 1,
                })
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/DoBandwidthTest"))?
                .into_inner();
            // Drain the stream so we measure the full round-trip including the response.
            while stream
                .next()
                .await
                .transpose()
                .map_err(|err| ApiError::tonic(&self.origin, err, "/DoBandwidthTest"))?
                .is_some()
            {}
            best = best.min(start.elapsed());
        }
        Ok(best)
    }

    /// Estimate the download bandwidth (bytes/second) from the server.
    ///
    /// Requests `num_bytes` of pseudo-random bytes via `/DoBandwidthTest`, subtracts `rtt` from
    /// the elapsed time, and divides by `num_bytes`.
    ///
    /// Returns `None` if the elapsed time is not greater than `rtt` (e.g. very small payloads on
    /// a fast loopback connection).
    pub async fn bandwidth_bytes_per_sec(
        &mut self,
        num_bytes: u64,
        rtt: std::time::Duration,
    ) -> ApiResult<Option<f64>> {
        let max = cloud_ext::MAX_BANDWIDTH_TEST_BYTES;
        if num_bytes > max {
            return Err(ApiError::invalid_arguments(
                &self.origin,
                format!("num_bytes ({num_bytes}) exceeds the maximum of {max}"),
            ));
        }

        let start = web_time::Instant::now();
        let mut stream = self
            .inner()
            .do_bandwidth_test(re_protos::cloud::v1alpha1::DoBandwidthTestRequest { num_bytes })
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/DoBandwidthTest"))?
            .into_inner();

        let mut received: u64 = 0;
        while let Some(item) = stream
            .next()
            .await
            .transpose()
            .map_err(|err| ApiError::tonic(&self.origin, err, "/DoBandwidthTest"))?
        {
            received += item.payload.len() as u64;
        }
        let elapsed = start.elapsed();

        let Some(transfer) = elapsed.checked_sub(rtt).filter(|t| !t.is_zero()) else {
            return Ok(None);
        };
        Ok(Some(received as f64 / transfer.as_secs_f64()))
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use re_protos::cloud::v1alpha1::rerun_cloud_service_client::RerunCloudServiceClient;

    use super::*;

    /// When the `features` cell is already populated, `supports_feature`
    /// must answer from the cache without going through the gRPC transport.
    ///
    /// We construct a `RedapClient` against a lazy channel
    /// pointing at an unrouteable address: any RPC against it would error
    /// (or hang past our test timeout). The test pre-populates the cell
    /// and then issues three `supports_feature` calls. If the cache is
    /// honored, all three return immediately; if it's bypassed, the
    /// transport call fails the test.
    #[tokio::test]
    async fn supports_feature_short_circuits_when_cache_is_populated() {
        // `connect_lazy` succeeds without doing any I/O; the failure
        // would only surface when an RPC actually flows through.
        let channel = tonic::transport::Channel::from_static("http://127.0.0.1:1").connect_lazy();
        let mut client = RedapClient::new(
            re_uri::Origin::test(),
            RerunCloudServiceClient::new(channel),
            None,
        );

        // Prime the cache exactly as a successful first-call would.
        client
            .features
            .set(vec![
                "per_segment_index_values".to_owned(),
                "future_X".to_owned(),
            ])
            .expect("freshly-constructed cell is empty");

        // Each of these must hit only the cache. If any of them attempts
        // an RPC, the unrouteable transport will error and fail the test.
        assert!(
            client
                .supports_feature("per_segment_index_values")
                .await
                .unwrap()
        );
        assert!(client.supports_feature("future_X").await.unwrap());
        assert!(!client.supports_feature("nonexistent").await.unwrap());
    }

    /// The cache is shared across clones via `Arc<OnceCell<_>>` — populating
    /// the cell on one clone makes it observable on the other.
    #[tokio::test]
    async fn features_cache_is_shared_across_clones() {
        let channel = tonic::transport::Channel::from_static("http://127.0.0.1:1").connect_lazy();
        let client_a = RedapClient::new(
            re_uri::Origin::test(),
            RerunCloudServiceClient::new(channel),
            None,
        );
        let mut client_b = client_a.clone();

        client_a
            .features
            .set(vec!["per_segment_index_values".to_owned()])
            .expect("freshly-constructed cell is empty");

        // The clone observes the same cached features and answers without
        // the transport.
        assert!(
            client_b
                .supports_feature("per_segment_index_values")
                .await
                .unwrap()
        );
        assert!(!client_b.supports_feature("nonexistent").await.unwrap());
    }

    /// Old server returns an empty `features` list. `supports_feature`
    /// must answer `Ok(false)` — never error — so callers can fall back
    /// to the pre-feature path.
    #[tokio::test]
    async fn supports_feature_returns_false_for_empty_features_list() {
        let channel = tonic::transport::Channel::from_static("http://127.0.0.1:1").connect_lazy();
        let mut client = RedapClient::new(
            re_uri::Origin::test(),
            RerunCloudServiceClient::new(channel),
            None,
        );

        client
            .features
            .set(vec![])
            .expect("freshly-constructed cell is empty");

        assert!(
            !client
                .supports_feature("per_segment_index_values")
                .await
                .unwrap()
        );
    }
}
