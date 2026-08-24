use arrow::array::RecordBatch;
use arrow::datatypes::{Schema as ArrowSchema, SchemaRef};
use itertools::Itertools as _;
use re_log_encoding::{Decodable as _, RawRrdManifest, ToApplication as _};
use re_log_types::EntryId;
use re_protos::EntryName;
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_protos::cloud::v1alpha1::ext::{
    self as cloud_ext, ETag, RrdManifestKey as RrdManifestKeyExt, SOURCE_CHANGED_MESSAGE,
    WatchEventsResponse, url_strip_query,
};
use re_protos::cloud::v1alpha1::ext::{
    CreateDatasetEntryResponse, CreateTableEntryRequest, DatasetDetails, DatasetEntry,
    EntryDetails, EntryDetailsUpdate, LanceTable, ProviderDetails, QueryDatasetRequest,
    QueryTasksOnCompletionRequest, QueryTasksRequest, ReadDatasetEntryResponse,
    ReadTableEntryResponse, RegisterTableResponse, TableDetails, TableEntry, TableInsertMode,
    UnregisterFromDatasetRequest, UpdateDatasetEntryRequest, UpdateDatasetEntryResponse,
    UpdateEntryRequest, UpdateEntryResponse, UpdateTableEntryRequest, UpdateTableEntryResponse,
    VersionResponse,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_client::RerunCloudServiceClient;
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::{
    RerunCloudService, RerunCloudServiceServer,
};
use re_protos::cloud::v1alpha1::{
    CancelTasksRequest, CreateDatasetEntryRequest, DeleteEntryRequest, EntryFilter, EntryKind,
    FetchChunksRequest, FindEntriesRequest, GetAssetsForSegmentResponse,
    GetDatasetManifestSchemaRequest, GetDatasetManifestSchemaResponse, GetDatasetSchemaRequest,
    GetRrdManifestResponse, GetSegmentTableSchemaRequest, GetSegmentTableSchemaResponse,
    QueryDatasetResponse, QueryTasksOnCompletionResponse, QueryTasksResponse,
    ReadDatasetEntryRequest, ReadTableEntryRequest, RrdManifestKey, ScanSegmentTableRequest,
    VersionRequest, WriteTableRequest,
};
use re_protos::common::v1alpha1::ext::{ScanParameters, SegmentId};
use re_protos::common::v1alpha1::{DataframePart, TaskId};
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_protos::missing_field;
use re_types_core::LayerName;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio_stream::{Stream, StreamExt as _};
use tonic::IntoStreamingRequest as _;
use tonic::codegen::{Body, StdError};
use url::Url;

use crate::{ApiError, ApiErrorKind, ApiResponseStream, ApiResult, TraceId, extract_trace_id};

/// Extension trait for [`tonic::Response`] that extracts both the inner value
/// and the server's trace-id in one step.
trait TonicResponseExt<T> {
    fn into_inner_and_trace_id(self) -> (T, Option<opentelemetry::TraceId>);
}

impl<T> TonicResponseExt<T> for tonic::Response<T> {
    fn into_inner_and_trace_id(self) -> (T, Option<opentelemetry::TraceId>) {
        let trace_id = extract_trace_id(self.metadata());
        (self.into_inner(), trace_id)
    }
}

pub type FetchChunksResponseStream =
    ApiResponseStream<re_protos::cloud::v1alpha1::FetchChunksResponse>;

pub type QueryDatasetResponseStream =
    ApiResponseStream<re_protos::cloud::v1alpha1::QueryDatasetResponse>;

type RedapHttpRequest = tonic::codegen::http::Request<tonic::body::Body>;
type RedapHttpResponse = tonic::codegen::http::Response<tonic::body::Body>;

pub type BoxedRedapClientStack =
    tower::util::BoxCloneSyncService<RedapHttpRequest, RedapHttpResponse, tonic::Status>;

/// Checks that a `Content-Range` value describes the exact inclusive byte range expected by the
/// request and, when present, a complete object length greater than the range end.
fn content_range_matches(value: &str, expected_start: u64, expected_end: u64) -> bool {
    let Some((unit, value)) = value.split_once(' ') else {
        return false;
    };
    let Some((range, complete_length)) = value.split_once('/') else {
        return false;
    };
    let Some((start, end)) = range.split_once('-') else {
        return false;
    };

    unit.eq_ignore_ascii_case("bytes")
        && start.parse::<u64>() == Ok(expected_start)
        && end.parse::<u64>() == Ok(expected_end)
        && (complete_length == "*"
            || complete_length
                .parse::<u64>()
                .is_ok_and(|complete_length| expected_end < complete_length))
}

async fn fetch_rrd_manifest_via_key(
    origin: &re_uri::Origin,
    manifest_key: RrdManifestKey,
    segment_id: &SegmentId,
    trace_id: Option<TraceId>,
) -> ApiResult<RawRrdManifest> {
    let RrdManifestKeyExt {
        location,
        layer,
        etag,
        direct_url,
    } = manifest_key.try_into().map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            "invalid /GetRrdManifest manifest key",
        )
    })?;

    let Some(direct_url) = direct_url else {
        return Err(ApiError::deserialization(
            origin,
            trace_id,
            "direct manifest key carries no direct_url to fetch",
        ));
    };

    let Some(range_end) = location
        .byte_span
        .len
        .checked_sub(1)
        .and_then(|length_minus_one| location.byte_span.start.checked_add(length_minus_one))
    else {
        return Err(ApiError::deserialization(
            origin,
            trace_id,
            "direct manifest byte range is empty or overflows u64",
        ));
    };

    let mut request = ehttp::Request::get(direct_url.as_str()).with_timeout(None);
    request.headers.insert(
        http::header::RANGE.as_str(),
        format!("bytes={}-{range_end}", location.byte_span.start),
    );
    let expected_etag = etag.filter(|etag| !etag.is_empty());
    if let Some(etag) = expected_etag.as_ref().and_then(ETag::as_if_match) {
        request
            .headers
            .insert(http::header::IF_MATCH.as_str(), etag);
    }

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

    let redacted_url = url_strip_query(direct_url.as_str());

    let response = response.map_err(|err| {
        let err = err.replace(direct_url.as_str(), redacted_url);
        ApiError::connection_with_source(
            origin,
            trace_id,
            std::io::Error::other(err),
            format!("failed to fetch RRD manifest directly\nURL: {redacted_url}"),
        )
    })?;

    let source_changed_error = || {
        ApiError::http_status_with_source(
            origin,
            trace_id,
            http::StatusCode::PRECONDITION_FAILED.as_u16(),
            std::io::Error::other(SOURCE_CHANGED_MESSAGE),
            format!("failed to fetch RRD manifest directly\nURL: {redacted_url}"),
        )
    };

    // HTTP permits servers to ignore `Range` and return `200 OK`, but this path requires the
    // requested range to be honored to avoid downloading or decoding the full RRD object.
    if response.status != http::StatusCode::PARTIAL_CONTENT.as_u16() {
        return if response.status == http::StatusCode::PRECONDITION_FAILED.as_u16() {
            Err(source_changed_error())
        } else {
            let layer = layer
                .as_deref()
                .map_or_else(String::new, |layer| format!("\nLayer: {layer}"));
            Err(ApiError::http_status(
                origin,
                trace_id,
                response.status,
                format!("failed to fetch RRD manifest directly{layer}\nURL: {redacted_url}"),
            ))
        };
    }

    let content_range = response.headers.get(http::header::CONTENT_RANGE.as_str());
    if !content_range.is_some_and(|content_range| {
        content_range_matches(content_range, location.byte_span.start, range_end)
    }) {
        return Err(ApiError::with_kind_and_source(
            origin,
            ApiErrorKind::InvalidServer,
            trace_id,
            std::io::Error::other(format!(
                "invalid Content-Range: {}",
                content_range.unwrap_or("missing")
            )),
            format!("failed to fetch RRD manifest directly\nURL: {redacted_url}"),
        ));
    }

    let expected_length = usize::try_from(location.byte_span.len).map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            "direct RRD manifest is too large for this client",
        )
    })?;
    if response.bytes.len() != expected_length {
        return Err(ApiError::deserialization(
            origin,
            trace_id,
            format!(
                "direct RRD manifest response had {} bytes, expected {expected_length}\nURL: {redacted_url}",
                response.bytes.len()
            ),
        ));
    }

    let rrd_footer = re_protos::log_msg::v1alpha1::RrdFooter::from_rrd_bytes(&response.bytes)
        .map_err(|err| {
            ApiError::deserialization_with_source(
                origin,
                trace_id,
                err,
                format!("failed decoding direct RRD footer\nURL: {redacted_url}"),
            )
        })?;

    // A footer may in theory contain manifests for several stores.
    // So we pick out the one that matches the requested segment_id!
    let rrd_manifest = rrd_footer
        .manifests
        .into_iter()
        .find(|manifest| {
            manifest
                .store_id
                .as_ref()
                .is_some_and(|store_id| store_id.recording_id == segment_id.as_str())
        })
        .ok_or_else(|| {
            ApiError::deserialization(origin,
                trace_id,
                format!(
                    "direct RRD footer did not contain a manifest for segment {segment_id}\nURL: {redacted_url}"
                ),
            )
        })?;

    let raw = rrd_manifest.to_application(()).map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            format!("failed parsing direct RRD manifest\nURL: {redacted_url}"),
        )
    })?;

    let Some(layer) = layer else {
        return Err(ApiError::deserialization(
            origin,
            trace_id,
            "direct manifest key carries no layer",
        ));
    };

    // `FetchChunks` needs the same `chunk_partition_id`/`rerun_partition_layer`/`chunk_key`
    // columns the inline `GetRrdManifest` path serves; the raw manifest alone doesn't carry them.
    // [`re_log_encoding::HubRrdManifest::try_from_raw`] handles this.
    let hub = re_log_encoding::HubRrdManifest::try_from_raw(
        &raw,
        segment_id,
        &layer,
        &location.url,
        expected_etag.as_ref(),
        None,
    )
    .map_err(|err| {
        ApiError::deserialization_with_source(
            origin,
            trace_id,
            err,
            format!("failed hub-extending direct RRD manifest\nURL: {redacted_url}"),
        )
    })?;

    // convert back to raw manifest so it can be interpreted
    // with no changes by downstream users
    Ok(hub.into_raw())
}

#[derive(Clone)]
pub struct SegmentQueryParams {
    pub dataset_id: EntryId,
    pub segment_id: SegmentId,
    pub include_static_data: bool,
    pub include_temporal_data: bool,
    pub generate_direct_urls: bool,
    pub query: Option<re_protos::cloud::v1alpha1::Query>,
}

/// Expose an ergonomic API over the gRPC redap client.
///
/// Implementation note: this type is generic so that it can be used with several client types. This
/// is useful for other projects which might have a different type (e.g. due to instrumentation).
/// Application code should use [`crate::ConnectionHandle`] to bind requests to an origin and
/// acquire a short-lived [`crate::ConnectionClient`] for RPCs.
//TODO(ab): this should NOT be `Clone`, to discourage callsites from holding on to a client for too
//long. However we have a bunch of places that needs to be fixed before we can do that.
#[derive(Clone)]
pub struct RedapClient<T> {
    /// The server this client talks to, named in every error it produces.
    origin: re_uri::Origin,

    inner: RerunCloudServiceClient<T>,

    /// Cached `VersionResponse.features` list. Populated lazily on the first
    /// `supports_feature` call and reused for the lifetime of this client
    /// (and any clones — `Arc` ensures clones share the same cache).
    ///
    /// Server features are stable per connection; if the server restarts
    /// with a different feature set, callers reconnect and get a fresh
    /// client (and a fresh cache).
    features: Arc<OnceCell<Vec<String>>>,

    /// Cache of certain chunks, like asset chunks.
    ///
    /// `None` disables chunk caching.
    chunk_cache: Option<crate::ChunkCacheHandle>,
}

impl<T> re_byte_size::SizeBytes for RedapClient<T> {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            origin,
            inner: _,
            features,
            chunk_cache,
        } = self;

        origin.heap_size_bytes()
            + features.get().map(|v| v.heap_size_bytes()).unwrap_or(0)
            + chunk_cache.heap_size_bytes()
    }
}

impl<T> RedapClient<T> {
    /// Create a new [`Self`].
    ///
    /// The `chunk_cache` is used to cache certain chunks, like from assets. Passing `None`
    /// disables chunk caching.
    ///
    /// Application code should acquire clients through [`crate::ConnectionHandle::client`]
    /// instead.
    pub fn new(
        origin: re_uri::Origin,
        client: RerunCloudServiceClient<T>,
        chunk_cache: Option<crate::ChunkCacheHandle>,
    ) -> Self {
        Self {
            origin,
            inner: client,
            features: Arc::new(OnceCell::new()),
            chunk_cache,
        }
    }

    /// The server this client talks to.
    #[inline]
    pub fn origin(&self) -> &re_uri::Origin {
        &self.origin
    }

    /// Get a mutable reference to the underlying generated gRPC client.
    //TODO(#10188): this should disappear once we have wrapper for all endpoints and the client code
    //is using them.
    pub fn inner(&mut self) -> &mut RerunCloudServiceClient<T> {
        &mut self.inner
    }

    /// Marks this chunks for caching.
    pub fn mark_asset_chunks(&self, chunk_ids: &[re_chunk::ChunkId]) {
        if let Some(chunk_cache) = &self.chunk_cache {
            chunk_cache.write().mark_chunks_cacheable(chunk_ids);
        }
    }
}

// `RerunCloudServiceClient<T>`'s derived `Debug` requires `T: Debug`, which doesn't hold for the
// type-erased `BoxedRedapClientStack`. Print only the cached features instead.
impl<T> std::fmt::Debug for RedapClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedapClient")
            .field("features", &self.features)
            .finish_non_exhaustive()
    }
}

// ---

/// Short-lived boxed-transport [`RedapClient`] for issuing RPCs.
///
/// Application code should acquire this from [`crate::ConnectionHandle::client`].
pub type ConnectionClient = RedapClient<BoxedRedapClientStack>;

/// Connection capabilities for a redap origin.
#[derive(Clone, Debug, re_byte_size::SizeBytes)]
pub struct Connection {
    pub client: ConnectionClient,
    pub analytics: Option<crate::ConnectionAnalyticsExporter>,
}

impl Connection {
    /// Create a connection backed by an in-process Rerun catalog implementation.
    pub fn from_service<T>(origin: re_uri::Origin, handler: Arc<T>) -> Self
    where
        T: RerunCloudService,
    {
        let service = <RerunCloudServiceServer<T> as tower::ServiceExt<RedapHttpRequest>>::map_err(
            RerunCloudServiceServer::from_arc(handler)
                .max_decoding_message_size(crate::MAX_DECODING_MESSAGE_SIZE),
            |err: std::convert::Infallible| match err {},
        );
        let client = RerunCloudServiceClient::new(tower::util::BoxCloneSyncService::new(service))
            .max_decoding_message_size(crate::MAX_DECODING_MESSAGE_SIZE);

        Self {
            client: RedapClient::new(origin, client, None),
            analytics: None,
        }
    }

    /// The server we are connected to
    pub fn origin(&self) -> &re_uri::Origin {
        self.client.origin()
    }
}

// ---

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body>,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    pub fn chunk_cache(&self) -> Option<&crate::ChunkCacheHandle> {
        self.chunk_cache.as_ref()
    }

    /// Uses the `/Version` endpoint for testing roundtrip time.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn ping(&mut self) -> ApiResult<()> {
        self.inner()
            .version(VersionRequest {})
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/Version failed"))
            .map(|_| ())
    }

    /// Returns version and deployment information from the server.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn version_info(&mut self) -> ApiResult<VersionResponse> {
        let response = self
            .inner()
            .version(VersionRequest {})
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/Version failed"))?
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
            .map_err(|err| ApiError::tonic(&self.origin, err, "/WhoAmI failed"))
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
                .map_err(|err| ApiError::tonic(&self.origin, err, "/DoBandwidthTest failed"))?
                .into_inner();
            // Drain the stream so we measure the full round-trip including the response.
            while stream
                .next()
                .await
                .transpose()
                .map_err(|err| ApiError::tonic(&self.origin, err, "/DoBandwidthTest stream error"))?
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
            .map_err(|err| ApiError::tonic(&self.origin, err, "/DoBandwidthTest failed"))?
            .into_inner();

        let mut received: u64 = 0;
        while let Some(item) =
            stream.next().await.transpose().map_err(|err| {
                ApiError::tonic(&self.origin, err, "/DoBandwidthTest stream error")
            })?
        {
            received += item.payload.len() as u64;
        }
        let elapsed = start.elapsed();

        let Some(transfer) = elapsed.checked_sub(rtt).filter(|t| !t.is_zero()) else {
            return Ok(None);
        };
        Ok(Some(received as f64 / transfer.as_secs_f64()))
    }

    /// Stream catalog lifecycle events as they happen on the server.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn watch_events(&mut self) -> ApiResult<ApiResponseStream<WatchEventsResponse>> {
        let response = self
            .inner()
            .watch_events(re_protos::cloud::v1alpha1::WatchEventsRequest {
                kinds: vec![re_protos::cloud::v1alpha1::EventKind::entry()],
            })
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/WatchEvents failed"))?;

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
                .map_err(|err| ApiError::tonic(&self.origin, err, "/FindEntries failed"))?,
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
            .map_err(|err| ApiError::tonic(&self.origin, err, "/DeleteEntry failed"))?;

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
                .map_err(|err| ApiError::tonic(&self.origin, err, "/UpdateEntry failed"))?,
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

    /// Get the Arrow schema for a dataset entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_schema(&mut self, entry_id: EntryId) -> ApiResult<ArrowSchema> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .get_dataset_schema(
                    tonic::Request::new(GetDatasetSchemaRequest {}).with_entry_id(entry_id),
                )
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/GetDatasetSchema failed"))?,
        );
        inner.schema().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /GetDatasetSchema response",
            )
        })
    }

    /// Create a new dataset entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn create_dataset_entry(
        &mut self,
        name: EntryName,
        entry_id: Option<EntryId>,
    ) -> ApiResult<DatasetEntry> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .create_dataset_entry(CreateDatasetEntryRequest {
                    name: Some(name.to_string()),
                    id: entry_id.map(Into::into),
                })
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/CreateDatasetEntry failed"))?,
        );
        let response: CreateDatasetEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /CreateDatasetEntry response",
            )
        })?;

        Ok(response.dataset)
    }

    /// Get information on a dataset entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn read_dataset_entry(&mut self, entry_id: EntryId) -> ApiResult<DatasetEntry> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .read_dataset_entry(
                    tonic::Request::new(ReadDatasetEntryRequest {}).with_entry_id(entry_id),
                )
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/ReadDatasetEntry failed"))?,
        );
        let response: ReadDatasetEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /ReadDatasetEntry response",
            )
        })?;

        Ok(response.dataset_entry)
    }

    /// Update the details of a dataset entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn update_dataset_entry(
        &mut self,
        entry_id: EntryId,
        dataset_details: DatasetDetails,
    ) -> ApiResult<DatasetEntry> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .update_dataset_entry(
                    tonic::Request::new(
                        UpdateDatasetEntryRequest {
                            id: entry_id,
                            dataset_details,
                        }
                        .into(),
                    )
                    .with_entry_id(entry_id),
                )
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/UpdateDatasetEntry failed"))?,
        );
        let response: UpdateDatasetEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /UpdateDatasetEntry response",
            )
        })?;

        Ok(response.dataset_entry)
    }

    /// Get information on a table entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn read_table_entry(&mut self, entry_id: EntryId) -> ApiResult<TableEntry> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .read_table_entry(
                    tonic::Request::new(ReadTableEntryRequest {
                        id: Some(entry_id.into()),
                    })
                    .with_entry_id(entry_id),
                )
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/ReadTableEntry failed"))?,
        );
        let response: ReadTableEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /ReadTableEntry response",
            )
        })?;

        Ok(response.table_entry)
    }

    /// Update the details of a table entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn update_table_entry(
        &mut self,
        entry_id: EntryId,
        table_details: TableDetails,
    ) -> ApiResult<TableEntry> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .update_table_entry(tonic::Request::new(
                    UpdateTableEntryRequest {
                        id: entry_id,
                        table_details,
                    }
                    .into(),
                ))
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/UpdateTableEntry failed"))?,
        );
        let response: UpdateTableEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /UpdateTableEntry response",
            )
        })?;

        Ok(response.table_entry)
    }

    //TODO(ab): accept entry name
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_segment_table_schema(&mut self, entry_id: EntryId) -> ApiResult<ArrowSchema> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .get_segment_table_schema(
                    tonic::Request::new(GetSegmentTableSchemaRequest {}).with_entry_id(entry_id),
                )
                .await
                .map_err(|err| {
                    ApiError::tonic(&self.origin, err, "GetSegmentTableSchema failed")
                })?,
        );
        inner
            .schema
            .ok_or_else(|| {
                let err = missing_field!(GetSegmentTableSchemaResponse, "schema");
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "missing field in /GetSegmentTableSchema response",
                )
            })?
            .try_into()
            .map_err(|err| {
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "failed parsing /GetSegmentTableSchema response",
                )
            })
    }

    /// Get a list of segment IDs for the given dataset entry ID.
    //TODO(ab): is there a way — and a reason — to not collect and instead return a stream?
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_segment_ids(&self, entry_id: EntryId) -> ApiResult<Vec<SegmentId>>
    where
        T: Clone,
    {
        const COLUMN_NAME: &str = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME;

        // Retry only the *open*: the server rejects `ScanSegmentTable` with `ResourceExhausted`
        // fail-fast at admission control, before the stream exists, so re-opening is idempotent.
        // Stream consumption below is intentionally outside the retry (consistent with
        // `query_dataset_raw`); once the stream is open it can't yield `ResourceExhausted`.
        let response = crate::with_retry_resource_exhausted("/ScanSegmentTable", || {
            let mut client = self.clone();
            async move {
                client
                    .inner()
                    .scan_segment_table(
                        tonic::Request::new(ScanSegmentTableRequest::with_columns([COLUMN_NAME]))
                            .with_entry_id(entry_id),
                    )
                    .await
                    .map_err(|err| ApiError::tonic(&self.origin, err, "/ScanSegmentTable failed"))
            }
        })
        .await?;

        let mut stream = ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/ScanSegmentTable",
        );
        let trace_id = stream.trace_id();

        let mut segment_ids = Vec::new();

        while let Some(resp) = stream.next().await {
            let record_batch: RecordBatch = resp?
                .data()
                .map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "failed parsing item from /ScanSegmentTable stream",
                    )
                })?
                .try_into()
                .map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "failed decoding item from /ScanSegmentTable stream",
                    )
                })?;

            let segment_id_column = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID
                .extract(&record_batch)
                .map_err(|err| {
                    ApiError::deserialization_quiver_from(
                        &self.origin,
                        trace_id,
                        err,
                        "/ScanSegmentTable stream",
                    )
                })?;

            segment_ids.extend(segment_id_column.into_iter_owned());
        }

        Ok(segment_ids)
    }

    //TODO(ab): accept entry name
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_manifest_schema(
        &mut self,
        entry_id: EntryId,
    ) -> ApiResult<ArrowSchema> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .get_dataset_manifest_schema(
                    tonic::Request::new(GetDatasetManifestSchemaRequest {}).with_entry_id(entry_id),
                )
                .await
                .map_err(|err| {
                    ApiError::tonic(&self.origin, err, "/GetDatasetManifestSchema failed")
                })?,
        );
        inner
            .schema
            .ok_or_else(|| {
                let err = missing_field!(GetDatasetManifestSchemaResponse, "schema");
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "missing field in /GetDatasetManifestSchema response",
                )
            })?
            .try_into()
            .map_err(|err| {
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "failed parsing /GetDatasetManifestSchema response",
                )
            })
    }

    /// Scan a dataset's manifest, keeping only the given columns.
    ///
    /// One row per (layer, segment), spread over as many batches as the server sends; see
    /// `ScanDatasetManifestDataframe` for the columns.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn scan_dataset_manifest(
        &mut self,
        entry_id: EntryId,
        columns: impl IntoIterator<Item = impl Into<String>>,
    ) -> ApiResult<Vec<RecordBatch>> {
        let response = self
            .inner()
            .scan_dataset_manifest(
                tonic::Request::new(
                    re_protos::cloud::v1alpha1::ScanDatasetManifestRequest::with_columns(columns),
                )
                .with_entry_id(entry_id),
            )
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/ScanDatasetManifest failed"))?;

        let mut stream = ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/ScanDatasetManifest",
        );
        let trace_id = stream.trace_id();

        let mut batches = Vec::new();

        while let Some(response) = stream.next().await {
            let batch: RecordBatch = response?
                .data()
                .map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "failed parsing item from /ScanDatasetManifest stream",
                    )
                })?
                .try_into()
                .map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "failed decoding item from /ScanDatasetManifest stream",
                    )
                })?;

            batches.push(batch);
        }

        Ok(batches)
    }

    /// Get the asset dataset that applies to a dataset, and the asset segments within it.
    ///
    /// Returns `None` if the dataset has no asset dataset, which means no assets were ever
    /// registered for it.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_assets_for_segment(
        &mut self,
        dataset_id: EntryId,
    ) -> ApiResult<Option<(EntryId, Vec<SegmentId>)>> {
        let response = self
            .inner()
            .get_assets_for_segment(
                tonic::Request::new(re_protos::cloud::v1alpha1::GetAssetsForSegmentRequest {})
                    .with_entry_id(dataset_id),
            )
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/GetAssetsForSegment failed"))?;

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

        Ok(assets_entry.map(|assets_entry| (assets_entry, asset_segment_ids)))
    }

    /// Stream the [`RawRrdManifest`] parts of a recording as they arrive from the server.
    ///
    /// Each item in the returned stream is a manifest part (a slice of the full manifest).
    /// Use [`RawRrdManifest::merge`] to combine parts because they may have different, overlapping schemas.
    ///
    /// The server may answer with manifest keys instead of inline manifests, in which case each
    /// part is fetched directly from the object store. In a browser that direct fetch only works
    /// when the bucket has a CORS configuration that allows it (see
    /// <https://rerun.io/docs/hub/bucket-cors>), so wasm clients fall back to
    /// server-provided manifests when the first part fails.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_rrd_manifest_stream(
        &mut self,
        dataset_id: EntryId,
        segment_id: SegmentId,
    ) -> ApiResult<ApiResponseStream<RawRrdManifest>> {
        if !cfg!(target_family = "wasm") {
            return self
                .get_rrd_manifest_stream_impl(dataset_id, segment_id, true)
                .await;
        }

        // In a browser, direct object-store reads only work when the bucket's CORS
        // configuration allows them, which cannot be assumed: if it fails,
        // ask for server-provided manifests instead.
        let mut stream = self
            .get_rrd_manifest_stream_impl(dataset_id, segment_id.clone(), true)
            .await?;
        let trace_id = stream.trace_id();
        match stream.next().await {
            Some(Ok(first)) => Ok(ApiResponseStream::new(
                self.origin.clone(),
                futures::stream::iter([Ok(first)]).chain(stream), // NOLINT: Stream::chain, not Iterator::chain
                trace_id,
            )),
            // Browsers report a CORS block as an opaque network failure, so `Connection` is
            // the narrowest kind that covers it. This costs a fallback even on other
            // types of network failures.
            Some(Err(err)) if err.kind == ApiErrorKind::Connection => {
                re_log::warn_once!(
                    "{}",
                    re_error::format_with_details(
                        "Failed to fetch an RRD footer directly from the object store, \
                         falling back to slower server-provided manifests. \
                         This usually means the bucket is missing a CORS configuration that \
                         allows the Web Viewer to read it — see \
                         https://rerun.io/docs/hub/bucket-cors for how to set it up.",
                        err.to_string(),
                    )
                );
                self.get_rrd_manifest_stream_impl(dataset_id, segment_id, false)
                    .await
            }
            Some(Err(err)) => Err(err),
            None => Ok(stream),
        }
    }

    async fn get_rrd_manifest_stream_impl(
        &mut self,
        dataset_id: EntryId,
        segment_id: SegmentId,
        generate_direct_urls: bool,
    ) -> ApiResult<ApiResponseStream<RawRrdManifest>> {
        let response = self
            .inner()
            .get_rrd_manifest(
                tonic::Request::new(re_protos::cloud::v1alpha1::GetRrdManifestRequest {
                    segment_id: Some(segment_id.clone().into()),
                    generate_direct_urls,
                })
                .with_entry_id(dataset_id),
            )
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/GetRrdManifest failed"))?;

        let stream = ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/GetRrdManifest",
        );
        let trace_id = stream.trace_id();
        let origin = self.origin.clone();
        let stream = stream.then(move |resp| {
            let segment_id = segment_id.clone();
            let origin = origin.clone();
            async move {
                let GetRrdManifestResponse {
                    rrd_manifest,
                    manifest_key,
                } = resp?;

                match (rrd_manifest, manifest_key) {
                    (Some(rrd_manifest), None) => rrd_manifest.to_application(()).map_err(|err| {
                        ApiError::deserialization_with_source(
                            &origin,
                            trace_id,
                            err,
                            "failed parsing inline /GetRrdManifest response",
                        )
                    }),
                    (None, Some(manifest_key)) => {
                        fetch_rrd_manifest_via_key(&origin, manifest_key, &segment_id, trace_id)
                            .await
                    }
                    (None, None) => Err(ApiError::deserialization(
                        &origin,
                        trace_id,
                        "/GetRrdManifest response contained neither a manifest nor a manifest key",
                    )),
                    (Some(_), Some(_)) => Err(ApiError::deserialization(
                        &origin,
                        trace_id,
                        "/GetRrdManifest response contained both a manifest and a manifest key",
                    )),
                }
            }
        });
        Ok(ApiResponseStream::new(
            self.origin.clone(),
            stream,
            trace_id,
        ))
    }

    /// Get the full [`RawRrdManifest`] of a recording, combined from all stream parts.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_rrd_manifest(
        &mut self,
        dataset_id: EntryId,
        segment_id: SegmentId,
    ) -> ApiResult<RawRrdManifest> {
        let stream = self.get_rrd_manifest_stream(dataset_id, segment_id).await?;
        let trace_id = stream.trace_id();

        futures::pin_mut!(stream);

        let mut rrd_manifest_parts = Vec::new();
        while let Some(part) = stream.next().await {
            rrd_manifest_parts.push(part?);
        }

        let Some(first) = rrd_manifest_parts.first() else {
            return Err(ApiError::deserialization(
                &self.origin,
                trace_id,
                "failed to parse the response for /GetRrdManifest (no data)",
            ));
        };
        if rrd_manifest_parts.len() == 1 {
            return Ok(rrd_manifest_parts.pop().expect("length was checked"));
        }

        RawRrdManifest::merge(first.store_id.clone(), rrd_manifest_parts).map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed merging /GetRrdManifest response parts",
            )
        })
    }

    /// Fetches all chunks ids for a specified segment.
    ///
    /// You can include/exclude static/temporal chunks,
    /// and limit the query to a time range.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn query_dataset_raw(
        &self,
        params: SegmentQueryParams,
    ) -> ApiResult<QueryDatasetResponseStream>
    where
        T: Clone,
    {
        // The server rejects `QueryDataset` with `ResourceExhausted` (fail-fast, before any work)
        // when its stream-concurrency limiter is saturated. Retry the *open* only; the returned
        // stream is consumed by the caller. `params` is cloned per attempt so we rebuild the
        // request fresh.
        crate::with_retry_resource_exhausted("/QueryDataset", || {
            let mut client = self.clone();
            let SegmentQueryParams {
                dataset_id,
                segment_id,
                include_static_data,
                include_temporal_data,
                query,
                generate_direct_urls,
            } = params.clone();

            async move {
                let query_request = QueryDatasetRequest {
                    segment_ids: vec![segment_id],
                    chunk_ids: vec![],
                    entity_paths: vec![],
                    select_all_entity_paths: true,
                    fuzzy_descriptors: vec![],
                    exclude_static_data: !include_static_data,
                    exclude_temporal_data: !include_temporal_data,
                    query: query.map(|q| q.try_into()).transpose().map_err(|err| {
                        ApiError::tonic(&self.origin, err, "failed building /QueryDataset request")
                    })?,
                    scan_parameters: Some(ScanParameters {
                        columns: FetchChunksRequest::required_column_names(),
                        ..Default::default()
                    }),
                    generate_direct_urls,
                };

                let response = client
                    .inner()
                    .query_dataset(
                        tonic::Request::new(query_request.into()).with_entry_id(dataset_id),
                    )
                    .await
                    .map_err(|err| ApiError::tonic(&self.origin, err, "/QueryDataset failed"))?;

                Ok(ApiResponseStream::from_tonic_response(
                    self.origin.clone(),
                    response,
                    "/QueryDataset",
                ))
            }
        })
        .await
    }

    /// Fetches all chunks ids for a specified segment.
    ///
    /// You can include/exclude static/temporal chunks,
    /// and limit the query to a time range.
    ///
    /// You can pass on the results to [`Self::query_dataset_chunk_index`].
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn query_dataset_chunk_index(
        &self,
        params: SegmentQueryParams,
    ) -> ApiResult<Vec<RecordBatch>>
    where
        T: Clone,
    {
        let stream = self.query_dataset_raw(params).await?;
        let trace_id = stream.trace_id();
        let responses: Vec<_> = stream.collect::<Vec<_>>().await.into_iter().try_collect()?;
        responses
            .into_iter()
            .map(|resp| {
                resp.data.ok_or_else(|| {
                    let err = missing_field!(QueryDatasetResponse, "data");
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "missing field in item in /QueryDataset response stream",
                    )
                })
            })
            .map(|batch| {
                arrow::array::RecordBatch::try_from(batch?).map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "failed converting to RecordBatch",
                    )
                })
            })
            .collect()
    }

    /// Input should be same schema as what [`Self::query_dataset_chunk_index`] returns.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn fetch_segment_chunks_by_id(
        &mut self,
        record_batch: &RecordBatch,
    ) -> ApiResult<FetchChunksResponseStream> {
        let (cached_chunks, to_fetch) = match &self.chunk_cache {
            Some(chunk_cache) => chunk_cache.split_cached(record_batch),
            None => (Vec::new(), Cow::Borrowed(record_batch)),
        };

        let cached = (!cached_chunks.is_empty()).then(|| {
            Ok(re_protos::cloud::v1alpha1::FetchChunksResponse {
                chunks: cached_chunks,
            })
        });
        let cached_stream = tokio_stream::iter(cached);

        if to_fetch.num_rows() == 0 {
            return Ok(ApiResponseStream::new(
                self.origin.clone(),
                cached_stream,
                None,
            ));
        }

        let fetch_chunks_request = FetchChunksRequest {
            chunk_infos: vec![DataframePart::from(&*to_fetch)],
        };

        let mut req = tonic::Request::new(fetch_chunks_request);
        req.set_timeout(crate::FETCH_CHUNKS_DEADLINE);
        let response = self
            .inner()
            .fetch_chunks(req)
            .await
            // NOTE: `ApiError::tonic` already extracts the trace-id from the error metadata.
            .map_err(|err| ApiError::tonic(&self.origin, err, "/FetchChunks failed"))?;

        let response =
            ApiResponseStream::from_tonic_response(self.origin.clone(), response, "/FetchChunks");
        let trace_id = response.trace_id();

        let chunk_cache_handle = self.chunk_cache.clone();
        let response = response.map(move |msg| {
            if let Some(chunk_cache) = &chunk_cache_handle
                && let Ok(msg) = &msg
            {
                chunk_cache.insert_cacheable(&msg.chunks);
            }

            msg
        });

        // Order does not have to be preserved, so fine to send cached first.
        Ok(ApiResponseStream::new(
            self.origin.clone(),
            cached_stream.merge(response),
            trace_id,
        ))
    }

    /// Fetches chunks for a specified partition and query.
    ///
    /// Convenience for [`Self::query_dataset_chunk_index`] + [`Self::fetch_segment_chunks_by_id`].
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn fetch_segment_chunks_by_query(
        &mut self,
        params: SegmentQueryParams,
    ) -> ApiResult<FetchChunksResponseStream>
    where
        T: Clone,
    {
        let stream = self.query_dataset_raw(params).await?;
        let query_trace_id = stream.trace_id();
        let responses: Vec<_> = stream.collect::<Vec<_>>().await.into_iter().try_collect()?;
        let chunk_info_batches: Vec<_> = responses
            .into_iter()
            .map(|resp| {
                resp.data.ok_or_else(|| {
                    let err = missing_field!(QueryDatasetResponse, "data");
                    ApiError::deserialization_with_source(
                        &self.origin,
                        query_trace_id,
                        err,
                        "missing field in item in /QueryDataset response stream",
                    )
                })
            })
            .try_collect()?;

        if chunk_info_batches.is_empty() {
            return Ok(ApiResponseStream::new(
                self.origin.clone(),
                tokio_stream::empty::<ApiResult<re_protos::cloud::v1alpha1::FetchChunksResponse>>(),
                None,
            ));
        }

        let fetch_chunks_request = FetchChunksRequest {
            chunk_infos: chunk_info_batches,
        };

        let mut req = tonic::Request::new(fetch_chunks_request);
        req.set_timeout(crate::FETCH_CHUNKS_DEADLINE);
        let response = self
            .inner()
            .fetch_chunks(req)
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/FetchChunks failed"))?;

        Ok(ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/FetchChunks",
        ))
    }

    /// Unregisters segments and layers from the dataset.
    ///
    /// This is an asynchronous operation, and returns a list of task ids.
    ///
    /// This method acts as a *product* filter:
    /// * empty `segments_to_drop` + empty `layers_to_drop`: invalid argument error
    /// * empty `segments_to_drop` + non-empty `layers_to_drop`: remove specified layers for *all* segments
    /// * non-empty `segments_to_drop` + empty `layers_to_drop`: remove *all* layers for specified segments
    /// * non-empty `segments_to_drop` + non-empty `layers_to_drop`: delete *all* specified layers for *all* specified segments
    ///
    /// If `force`, deletion will go through regardless of the segments/layers' current statuses.
    /// This is only useful in the very specific, catatrophic scenario where the contents of the
    /// task queue were lost and some tasks are now stuck in `status=pending` forever.
    /// Do not use this unless you know exactly what you're doing.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn unregister_from_dataset(
        &mut self,
        dataset_id: EntryId,
        segments_to_drop: Vec<SegmentId>,
        layers_to_drop: Vec<LayerName>,
        force: bool,
    ) -> ApiResult<(Option<TraceId>, Vec<TaskId>)> {
        let req = tonic::Request::new(
            UnregisterFromDatasetRequest {
                segments_to_drop,
                layers_to_drop,
                force,
            }
            .into(),
        )
        .with_entry_id(dataset_id);

        use futures::TryStreamExt as _;
        let response = self
            .inner()
            .unregister_from_dataset(req)
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/UnregisterFromDataset failed"))?;

        let trace_id = extract_trace_id(response.metadata());

        let stream = ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/UnregisterFromDataset",
        );
        let responses: Vec<_> = stream.try_collect().await?;

        let tasks = responses
            .into_iter()
            .filter_map(|resp| resp.task_id)
            .collect();

        Ok((trace_id, tasks))
    }

    /// Register a foreign Lance table to a new table entry in the catalog.
    //TODO(ab): in the future, we will probably support my types of tables (parquet on S3, etc.)
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn register_table(
        &mut self,
        name: EntryName,
        url: url::Url,
    ) -> ApiResult<TableEntry> {
        let request = cloud_ext::RegisterTableRequest {
            name,
            provider_details: ProviderDetails::LanceTable(LanceTable { table_url: url }),
        };

        let origin = self.origin.clone();
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .register_table(tonic::Request::new(request.try_into().map_err(|err| {
                    ApiError::serialization_with_source(
                        &origin,
                        err,
                        "failed building /RegisterTable request",
                    )
                })?))
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/RegisterTable failed"))?,
        );
        let response: RegisterTableResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /RegisterTable response",
            )
        })?;

        Ok(response.table_entry)
    }

    #[expect(clippy::fn_params_excessive_bools)] // TODO(emilk): remove bool parameters
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn do_maintenance(
        &mut self,
        dataset_id: EntryId,
        optimize_indexes: bool,
        retrain_indexes: bool,
        compact_fragments: bool,
        cleanup_before: Option<jiff::Timestamp>,
        unsafe_allow_recent_cleanup: bool,
    ) -> ApiResult {
        self.inner()
            .do_maintenance(
                tonic::Request::new(
                    cloud_ext::DoMaintenanceRequest {
                        optimize_indexes,
                        retrain_indexes,
                        compact_fragments,
                        cleanup_before,
                        gc_object_store: false,
                        unsafe_allow_recent_cleanup,
                    }
                    .into(),
                )
                .with_entry_id(dataset_id),
            )
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/DoMaintenance failed"))?;

        Ok(())
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn do_global_maintenance(&mut self) -> ApiResult {
        self.inner()
            .do_global_maintenance(tonic::Request::new(
                re_protos::cloud::v1alpha1::DoGlobalMaintenanceRequest {},
            ))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/DoGlobalMaintenance failed"))?;

        Ok(())
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_table_names(&mut self) -> ApiResult<Vec<EntryName>> {
        Ok(self
            .find_entries(re_protos::cloud::v1alpha1::EntryFilter {
                // Pass both `entry_kind` (deprecated) and `entry_kinds`
                // to be compatible with old Hub versions.
                // Drop `entry_kind` when no customer has a 0.14 deployment
                // or older of Rerun Hub.
                entry_kind: Some(EntryKind::Table.into()),
                entry_kinds: vec![EntryKind::Table.into()],
                ..Default::default()
            })
            .await?
            .into_iter()
            .map(|entry| entry.name.clone())
            .collect())
    }

    // -- Tasks API --
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn query_tasks_on_completion(
        &mut self,
        task_ids: Vec<TaskId>,
        timeout: std::time::Duration,
    ) -> ApiResult<ApiResponseStream<QueryTasksOnCompletionResponse>> {
        let q = QueryTasksOnCompletionRequest { task_ids, timeout };
        let origin = self.origin.clone();
        let response = self
            .inner()
            .query_tasks_on_completion(tonic::Request::new(q.try_into().map_err(|err| {
                ApiError::serialization_with_source(
                    &origin,
                    err,
                    "failed building /QueryTasksOnCompletion request",
                )
            })?))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/QueryTasksOnCompletion failed"))?;
        Ok(ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/QueryTasksOnCompletion",
        ))
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn cancel_tasks(&mut self, task_ids: Vec<TaskId>) -> ApiResult {
        self.inner()
            .cancel_tasks(CancelTasksRequest { ids: task_ids })
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/CancelTasks failed"))?;

        Ok(())
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn query_tasks(&mut self, task_ids: Vec<TaskId>) -> ApiResult<QueryTasksResponse> {
        let q = QueryTasksRequest { task_ids };
        let origin = self.origin.clone();
        let response = self
            .inner()
            .query_tasks(tonic::Request::new(q.try_into().map_err(|err| {
                ApiError::serialization_with_source(
                    &origin,
                    err,
                    "failed building /QueryTasks request",
                )
            })?))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/QueryTasks failed"))?
            .into_inner();
        Ok(response)
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
                .map_err(|err| ApiError::tonic(&self.origin, err, "/FindEntries failed"))?,
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
                        "/FindEntries failed",
                    )
                })
            })
            .transpose()
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub async fn write_table(
        &mut self,
        stream: impl Stream<Item = RecordBatch> + Send + 'static,
        table_id: EntryId,
        insert_mode: TableInsertMode,
    ) -> ApiResult {
        let insert_mode = re_protos::cloud::v1alpha1::TableInsertMode::from(insert_mode).into();
        let stream = stream
            .map(move |batch| WriteTableRequest {
                dataframe_part: Some(batch.into()),
                insert_mode,
            })
            .into_streaming_request()
            .with_entry_id(table_id);

        self.inner()
            .write_table(stream)
            .await
            .map(|_| ())
            .map_err(|err| ApiError::tonic(&self.origin, err, "/WriteTable failed"))
    }

    /// Create a table entry.
    ///
    /// NOTE: if `url` is provided, the caller must ensure that it is writable and yet unused.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn create_table_entry(
        &mut self,
        name: EntryName,
        url: Option<Url>,
        schema: SchemaRef,
    ) -> ApiResult<TableEntry> {
        let provider_details =
            url.map(|url| ProviderDetails::LanceTable(LanceTable { table_url: url }));
        let request = CreateTableEntryRequest {
            name,
            schema: schema.as_ref().clone(),
            provider_details,
        };

        let origin = self.origin.clone();
        let (resp, trace_id) = self
            .inner()
            .create_table_entry(tonic::Request::new(request.try_into().map_err(|err| {
                ApiError::internal_with_source(&origin, None, err, "/CreateTableEntry failed")
            })?))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "failed to create table"))?
            .into_inner_and_trace_id();

        resp.table
            .ok_or_else(|| {
                ApiError::deserialization(
                    &self.origin,
                    trace_id,
                    "/CreateTable failed: entry ID not set in response",
                )
            })?
            .try_into()
            .map_err(|err| {
                ApiError::internal_with_source(&self.origin, trace_id, err, "/CreateTable failed")
            })
    }

    /// Look up a dataset entry by name, returning its id if it exists.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn find_dataset_by_name(&mut self, name: &EntryName) -> ApiResult<Option<EntryId>> {
        let entries = match self
            .find_entries(EntryFilter {
                name: Some(name.to_string()),
                // Pass both `entry_kind` (deprecated) and `entry_kinds`
                // to be compatible with old Hub versions.
                // Drop `entry_kind` when no customer has a 0.14 deployment
                // or older of Rerun Hub.
                entry_kind: Some(EntryKind::Dataset.into()),
                entry_kinds: vec![EntryKind::Dataset.into()],
                ..Default::default()
            })
            .await
        {
            Ok(entries) => entries,
            Err(err) if err.kind == ApiErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        Ok(entries.into_iter().next().map(|entry| entry.id))
    }

    /// Find the dataset named `name`, creating it if it doesn't exist yet.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn find_or_create_dataset(&mut self, name: &EntryName) -> ApiResult<EntryId> {
        if let Some(id) = self.find_dataset_by_name(name).await? {
            return Ok(id);
        }

        match self.create_dataset_entry(name.clone(), None).await {
            Ok(dataset) => Ok(dataset.details.id),

            // Created concurrently between our lookup and our create.
            Err(err) if err.kind == ApiErrorKind::AlreadyExists => {
                self.find_dataset_by_name(name).await?.ok_or_else(|| {
                    ApiError::invalid_arguments(
                        &self.origin,
                        format!("dataset '{name}' disappeared while registering"),
                    )
                })
            }

            Err(err) => Err(err),
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn direct_rrd_manifest_response_metadata_is_validated() {
        assert!(content_range_matches("bytes 10-19/100", 10, 19));
        assert!(content_range_matches("BYTES 10-19/*", 10, 19));
        assert!(!content_range_matches("bytes 11-20/100", 10, 19));
        assert!(!content_range_matches("bytes 10-19/19", 10, 19));
        assert!(!content_range_matches("invalid", 10, 19));
    }

    #[tokio::test]
    async fn direct_rrd_manifest_fetch_uses_range_and_etag() {
        use re_log_encoding::{Encodable as _, ToTransport as _};

        let raw_manifest = RawRrdManifest::build_in_memory_from_chunks(
            re_log_types::StoreId::recording("test", "recording"),
            std::iter::empty::<&re_chunk::Chunk>(),
        )
        .unwrap();
        let other_manifest = RawRrdManifest::build_in_memory_from_chunks(
            re_log_types::StoreId::recording("test", "other"),
            std::iter::empty::<&re_chunk::Chunk>(),
        )
        .unwrap();
        let rrd_footer = re_log_encoding::RrdFooter {
            manifests: std::collections::HashMap::from([
                // Add a second manifest to ensure we pick out the right one.
                (other_manifest.store_id.clone(), other_manifest),
                (raw_manifest.store_id.clone(), raw_manifest.clone()),
            ]),
        };
        let mut manifest_bytes = Vec::new();
        rrd_footer
            .to_transport(())
            .unwrap()
            .to_rrd_bytes(&mut manifest_bytes)
            .unwrap();

        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let offset = 123;
        let range_end = offset + manifest_bytes.len() as u64 - 1;
        let response_bytes = manifest_bytes.clone();
        let server_thread = std::thread::Builder::new()
            .name("direct-manifest-test-server".to_owned())
            .spawn(move || {
                let request = server.recv().unwrap();
                let header = |name| {
                    request
                        .headers()
                        .iter()
                        .find(|header| header.field.equiv(name))
                        .map(|header| header.value.as_str().to_owned())
                };
                let range = header(http::header::RANGE.as_str());
                let if_match = header(http::header::IF_MATCH.as_str());
                request
                    .respond(
                        tiny_http::Response::from_data(response_bytes)
                            .with_status_code(tiny_http::StatusCode(
                                http::StatusCode::PARTIAL_CONTENT.as_u16(),
                            ))
                            .with_header(
                                tiny_http::Header::from_bytes(
                                    http::header::CONTENT_RANGE.as_str(),
                                    format!("bytes {offset}-{range_end}/{}", range_end + 1),
                                )
                                .unwrap(),
                            )
                            .with_header(
                                tiny_http::Header::from_bytes(
                                    http::header::ETAG.as_str(),
                                    "\"different-etag\"",
                                )
                                .unwrap(),
                            ),
                    )
                    .unwrap();
                (range, if_match)
            })
            .unwrap();

        let manifest_key = RrdManifestKey {
            location: Some(re_protos::cloud::v1alpha1::RrdChunkLocation {
                url: Some("s3://bucket/recording.rrd".to_owned()),
                offset: Some(offset),
                length: Some(manifest_bytes.len() as u64),
            }),
            layer: Some("base".to_owned()),
            etag: Some("\"registered-etag\"".to_owned()),
            direct_url: Some(format!(
                "http://{address}/recording.rrd?X-Amz-Signature=secret"
            )),
        };

        let segment_id = SegmentId::new("recording".to_owned());
        let layer = LayerName::base();
        let canonical_url = url::Url::parse("s3://bucket/recording.rrd").expect("valid s3 url");
        let etag = ETag::new("\"registered-etag\"");

        let fetched =
            fetch_rrd_manifest_via_key(&re_uri::Origin::test(), manifest_key, &segment_id, None)
                .await
                .expect("direct manifest fetch succeeds");
        assert_eq!(fetched.store_id, raw_manifest.store_id);
        assert_eq!(
            fetched.sorbet_schema_sha256,
            raw_manifest.sorbet_schema_sha256
        );

        let expected = re_log_encoding::HubRrdManifest::try_from_raw(
            &raw_manifest,
            &segment_id,
            &layer,
            &canonical_url,
            Some(&etag),
            None,
        )
        .expect("hub-extending a zero-row in-memory manifest cannot fail")
        .into_raw();
        assert_eq!(fetched.data, expected.data);
        for hub_column in [
            re_log_encoding::HubRrdManifest::FIELD_CHUNK_PARTITION_ID,
            re_log_encoding::HubRrdManifest::FIELD_RERUN_PARTITION_LAYER,
            re_log_encoding::HubRrdManifest::FIELD_CHUNK_KEY,
        ] {
            assert!(
                fetched.data.column_by_name(hub_column).is_some(),
                "fetched manifest is missing hub column '{hub_column}'"
            );
        }

        let (range, if_match) = server_thread.join().unwrap();
        assert_eq!(
            range.as_deref(),
            Some(format!("bytes={offset}-{range_end}").as_str())
        );
        assert_eq!(if_match.as_deref(), Some("\"registered-etag\""));
    }

    #[tokio::test]
    async fn direct_rrd_manifest_fetch_maps_precondition_failure_without_leaking_query() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let server_thread = std::thread::Builder::new()
            .name("direct-manifest-test-server".to_owned())
            .spawn(move || {
                let request = server.recv().unwrap();
                request
                    .respond(tiny_http::Response::empty(tiny_http::StatusCode(
                        http::StatusCode::PRECONDITION_FAILED.as_u16(),
                    )))
                    .unwrap();
            })
            .unwrap();

        let manifest_key = RrdManifestKey {
            location: Some(re_protos::cloud::v1alpha1::RrdChunkLocation {
                url: Some("s3://bucket/manifest".to_owned()),
                offset: Some(0),
                length: Some(1),
            }),
            layer: None,
            etag: Some("\"old-etag\"".to_owned()),
            direct_url: Some(format!("http://{address}/manifest?secret=credential")),
        };

        let err = fetch_rrd_manifest_via_key(
            &re_uri::Origin::test(),
            manifest_key,
            &SegmentId::new("recording".to_owned()),
            None,
        )
        .await
        .unwrap_err();
        server_thread.join().unwrap();

        assert_eq!(err.kind, ApiErrorKind::FailedPrecondition);
        assert!(err.to_string().contains(SOURCE_CHANGED_MESSAGE));
        assert!(!err.to_string().contains("credential"));
    }

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
