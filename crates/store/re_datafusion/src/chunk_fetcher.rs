//! Chunk fetching strategies: direct URL (HTTP Range) and gRPC.

use std::collections::BTreeMap;
use std::time::Duration;
use std::{error::Error as _, fmt::Write as _};

use arrow::array::{
    Array as _, ArrayAccessor as _, BinaryArray, DictionaryArray, RecordBatch, StringArray,
    UInt64Array,
};
use arrow::datatypes::Int32Type;
use futures::StreamExt as _;
use tonic::IntoRequest as _;
use tracing::Instrument as _;

use re_dataframe::external::re_chunk::Chunk;
use re_protos::cloud::v1alpha1::ext::{
    ChunkKey, ETag, RrdChunkLocation, SOURCE_CHANGED_MESSAGE, url_strip_query,
};
use re_protos::cloud::v1alpha1::{FetchChunksRequest, QueryDatasetResponse};
use re_redap_client::ApiResult;

use crate::analytics::{DirectFetchFailureReason, PendingQueryAnalytics, TaskFetchStats};
use crate::dataframe_query_common::DataframeClientAPI;

// --- Telemetry ---

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod metrics {
    use std::sync::OnceLock;

    use opentelemetry::{KeyValue, metrics::Counter};

    struct ChunkFetchMetrics {
        /// Counts direct fetch outcomes: result = `success` | `failure`
        direct_result: Counter<u64>,

        /// Counts bytes fetched, method = `direct` | `grpc`
        bytes_fetched: Counter<u64>,

        /// Counts gRPC fetches for chunks without direct URLs
        grpc_no_direct_urls: Counter<u64>,
    }

    fn get() -> &'static ChunkFetchMetrics {
        static INSTANCE: OnceLock<ChunkFetchMetrics> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            let meter = opentelemetry::global::meter("chunk_fetch");
            ChunkFetchMetrics {
                direct_result: meter
                    .u64_counter("chunk_fetch.direct.result")
                    .with_description("Direct fetch outcomes")
                    .build(),
                bytes_fetched: meter
                    .u64_counter("chunk_fetch.bytes")
                    .with_description("Bytes fetched for chunk data")
                    .with_unit("B")
                    .build(),
                grpc_no_direct_urls: meter
                    .u64_counter("chunk_fetch.grpc_no_direct_urls")
                    .with_description("gRPC fetches for chunks without direct URLs")
                    .build(),
            }
        })
    }

    /// Record when some number of bytes has been successfully fetched directly from object storage.
    pub fn record_direct_success(bytes: u64) {
        let m = get();
        m.direct_result
            .add(1, &[KeyValue::new("result", "success")]);
        m.bytes_fetched
            .add(bytes, &[KeyValue::new("method", "direct")]);
    }

    /// Record a direct fetch failure after retries were exhausted.
    ///
    /// `reason` should be one of: `"timeout"`, `"http_4xx"`, `"http_5xx"`,
    /// `"connection"`, `"decode"`, `"other"`.
    pub fn record_direct_failure(reason: &str) {
        let m = get();
        m.direct_result.add(
            1,
            &[
                KeyValue::new("result", "failure"),
                KeyValue::new("reason", reason.to_owned()),
            ],
        );
    }

    /// Record a gRPC fetch when no direct URLs were available in the batch.
    pub fn record_grpc_no_direct_urls(bytes: u64) {
        let m = get();
        m.grpc_no_direct_urls.add(1, &[]);
        m.bytes_fetched
            .add(bytes, &[KeyValue::new("method", "grpc")]);
    }
}

/// Chunks tagged with their segment ID.
pub type ChunksWithSegment = Vec<(Chunk, Option<String>)>;

pub type SortedChunksWithSegment = (String, Vec<Chunk>);

/// Maximum size of a single merged HTTP Range request (16 MB, matching server).
const MAX_MERGED_RANGE_SIZE: usize = 16 * 1024 * 1024;

/// Number of times to retry direct fetch on transient errors before returning a hard error.
const DIRECT_FETCH_MAX_RETRIES: usize = 10;

// --- Range merging types ---

/// Where a single chunk lives within a merged range response.
struct ChunkInMergedRange {
    /// Index of this chunk in the original `RecordBatch` (used to preserve ordering).
    original_row_index: usize,

    /// Byte offset of this chunk within the merged response body.
    offset_in_merged: usize,

    /// Byte length of this chunk.
    length: usize,
}

/// A single HTTP Range request that may cover multiple adjacent chunks.
struct MergedRangeRequest {
    /// The presigned URL to fetch from.
    url: String,

    /// Absolute byte range start within the file (inclusive).
    file_range_start: usize,

    /// Absolute byte range end within the file (exclusive).
    file_range_end: usize,

    /// Individual chunks to extract from the merged response.
    chunks: Vec<ChunkInMergedRange>,

    /// Segment ID the chunks in this merged range belong to.
    segment_id: Option<String>,

    /// `ETag` the manifest registered for the source object, when known.
    expected_etag: Option<ETag>,

    /// Wall-clock registration time of the parent segment, when known.
    registration_time: Option<jiff::Timestamp>,
}

/// Discriminant on [`DirectFetchError`].
///
/// Currently only used for `SourceChanged` errors, but may
/// be expanded in the future to stop relying on string message
/// matching for error classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectFetchErrorKind {
    Generic,
    SourceChanged,
}

/// Error from a direct URL fetch attempt. Retried up to [`DIRECT_FETCH_MAX_RETRIES`] times.
#[derive(Debug)]
pub struct DirectFetchError {
    msg: String,
    retryable: bool,
    pub kind: DirectFetchErrorKind,
}

impl DirectFetchError {
    fn new(msg: String, retryable: bool) -> Self {
        Self {
            msg,
            retryable,
            kind: DirectFetchErrorKind::Generic,
        }
    }

    /// The source object backing this fetch has changed since the dataset was
    /// registered. Non-retryable: re-trying produces the same drift.
    fn source_changed(segment_id: Option<&str>) -> Self {
        let msg = if let Some(id) = segment_id {
            format!("{SOURCE_CHANGED_MESSAGE}: {id}")
        } else {
            SOURCE_CHANGED_MESSAGE.to_owned()
        };
        Self {
            msg,
            retryable: false,
            kind: DirectFetchErrorKind::SourceChanged,
        }
    }
}

impl std::fmt::Display for DirectFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for DirectFetchError {}

/// Returns `true` if the batch contains at least one non-null direct URL.
pub fn batch_has_any_direct_urls(batch: &RecordBatch) -> bool {
    batch
        .column_by_name(QueryDatasetResponse::FIELD_DIRECT_URL)
        .is_some_and(|col| col.null_count() < col.len())
}

/// Split a batch into (direct-URL rows, non-URL rows).
///
/// Either half is `None` if it would have zero rows.
pub fn split_batch_by_direct_url(
    batch: &RecordBatch,
) -> (Option<RecordBatch>, Option<RecordBatch>) {
    re_tracing::profile_function!();
    use arrow::compute::{filter_record_batch, is_not_null, not};

    let Some(url_col) = batch.column_by_name(QueryDatasetResponse::FIELD_DIRECT_URL) else {
        return (None, Some(batch.clone()));
    };

    let has_url = is_not_null(url_col).expect("is_not_null on direct_url column");
    let no_url = not(&has_url).expect("boolean not");

    let direct_batch = if has_url.true_count() > 0 {
        Some(filter_record_batch(batch, &has_url).expect("filter_record_batch for direct URL rows"))
    } else {
        None
    };

    let grpc_batch = if no_url.true_count() > 0 {
        Some(filter_record_batch(batch, &no_url).expect("filter_record_batch for gRPC rows"))
    } else {
        None
    };

    (direct_batch, grpc_batch)
}

/// Sum of `chunk_byte_len` values in a batch (best-effort, returns 0 on missing column).
pub fn batch_byte_size(batch: &RecordBatch) -> u64 {
    batch
        .column_by_name(QueryDatasetResponse::FIELD_CHUNK_BYTE_LENGTH)
        .and_then(|c| c.as_any().downcast_ref::<UInt64Array>())
        .map(|arr| arr.iter().map(|v| v.unwrap_or(0)).sum())
        .unwrap_or(0)
}

/// Sum of `chunk_byte_size_uncompressed` values in a batch, if the column is present.
///
/// Returns `None` when the server did not supply uncompressed sizes (older server or
/// the column was not projected).
pub fn batch_byte_size_uncompressed(batch: &RecordBatch) -> Option<u64> {
    batch
        .column_by_name(QueryDatasetResponse::FIELD_CHUNK_BYTE_LENGTH_UNCOMPRESSED)
        .and_then(|c| c.as_any().downcast_ref::<UInt64Array>())
        .map(|arr| arr.iter().map(|v| v.unwrap_or(0)).sum())
}

/// Fetch a batch of chunks via direct URLs.
///
/// Individual requests are retried up to [`DIRECT_FETCH_MAX_RETRIES`] times on transient errors.
///
/// `stats` is the caller's per-task accumulator; `pending` is used only for
/// recording the one-shot terminal failure reason on the shared state.
#[tracing::instrument(level = "info", skip_all, fields(num_chunks, byte_size))]
pub async fn fetch_batch_direct(
    batch: &RecordBatch,
    http_client: &reqwest::Client,
    stats: &mut TaskFetchStats,
    pending: Option<&PendingQueryAnalytics>,
) -> ApiResult<Vec<ChunksWithSegment>> {
    #[cfg(not(target_arch = "wasm32"))]
    let byte_size = batch_byte_size(batch);

    let span = tracing::Span::current();
    span.record("num_chunks", batch.num_rows());
    #[cfg(not(target_arch = "wasm32"))]
    span.record("byte_size", byte_size);

    match fetch_batch_via_direct_urls(http_client, batch, stats).await {
        Ok(chunks) => {
            #[cfg(not(target_arch = "wasm32"))]
            metrics::record_direct_success(byte_size);
            Ok(chunks)
        }
        Err(err) => {
            let reason = DirectFetchFailureReason::classify(&err);
            if let Some(pending) = pending {
                pending.record_direct_terminal_failure(reason);
            }
            #[cfg(not(target_arch = "wasm32"))]
            metrics::record_direct_failure(reason.as_str());
            Err(re_redap_client::ApiError::connection_with_source(
                None,
                err,
                "fetching chunks via direct URLs",
            ))
        }
    }
}

impl DirectFetchFailureReason {
    /// Classify a `DirectFetchError`.
    ///
    /// Source-changed errors are matched on the typed [`DirectFetchErrorKind`]
    /// discriminant; everything else still falls through to message
    /// pattern-matching for now.
    fn classify(err: &DirectFetchError) -> Self {
        if err.kind == DirectFetchErrorKind::SourceChanged {
            return Self::SourceChanged;
        }
        let msg = &err.msg;
        if msg.contains("timed out") || msg.contains("Timeout") {
            Self::Timeout
        } else if msg.contains("status 4") {
            Self::Http4xx
        } else if msg.contains("status 5") {
            Self::Http5xx
        } else if msg.contains("connection") || msg.contains("dns") || msg.contains("connect") {
            Self::Connection
        } else if msg.contains("decode")
            || msg.contains("from_rrd_bytes")
            || msg.contains("from_record_batch")
        {
            Self::Decode
        } else {
            Self::Other
        }
    }
}

/// Fetch a group of batches using the gRPC `FetchChunks` proxy.
pub async fn fetch_batch_group_via_grpc<T: DataframeClientAPI>(
    batch_group: &[RecordBatch],
    client: &T,
) -> ApiResult<Vec<ChunksWithSegment>> {
    let mut all_chunks = Vec::new();

    let mut client = client.clone();
    for batch in batch_group {
        let chunk_info: re_protos::common::v1alpha1::DataframePart = batch.clone().into();

        let fetch_chunks_request = FetchChunksRequest {
            chunk_infos: vec![chunk_info],
        };

        let mut req = fetch_chunks_request.into_request();
        req.set_timeout(re_redap_client::FETCH_CHUNKS_DEADLINE);
        let response = client
            .fetch_chunks(req)
            .instrument(tracing::trace_span!("batched_fetch_chunks"))
            .await
            .map_err(|err| re_redap_client::ApiError::tonic(err, "FetchChunks request failed"))?;

        let response_stream =
            re_redap_client::ApiResponseStream::from_tonic_response(response, "/FetchChunks");

        let chunk_stream =
            re_redap_client::fetch_chunks_response_to_chunk_and_segment_id(response_stream);

        let batch_chunks: Vec<ApiResult<ChunksWithSegment>> = chunk_stream.collect().await;
        for chunk_result in batch_chunks {
            all_chunks.push(chunk_result?);
        }
    }

    Ok(all_chunks)
}

fn classify_http_status(status: reqwest::StatusCode) -> DirectFetchError {
    DirectFetchError {
        msg: format!("HTTP request returned status {status}"),
        retryable: status_retryable(status),
        kind: DirectFetchErrorKind::Generic,
    }
}

fn status_retryable(status: reqwest::StatusCode) -> bool {
    !matches!(
        status,
        reqwest::StatusCode::BAD_REQUEST
            | reqwest::StatusCode::UNAUTHORIZED
            | reqwest::StatusCode::FORBIDDEN
            | reqwest::StatusCode::METHOD_NOT_ALLOWED
    )
}

impl From<reqwest::Error> for DirectFetchError {
    fn from(err: reqwest::Error) -> Self {
        let mut msg = match err.status() {
            Some(status) => {
                format!("HTTP request failed with status {status}: {err}")
            }
            None => format!("HTTP request failed: {err}"),
        };

        let retryable = err.status().is_none_or(status_retryable);

        if let Some(source) = err.source() {
            write!(msg, " ({source})").expect("Can append");
        }

        Self {
            msg,
            retryable,
            kind: DirectFetchErrorKind::Generic,
        }
    }
}

// --- Range merging helpers (ported from rrd_mapper.rs) ---

/// Returns the optimal gap size for merging adjacent byte ranges.
/// Uses 25% of average chunk size — merging across a gap "wastes" at most 25% extra bandwidth.
fn calculate_optimal_gap_size(ranges: &[(u64, u64)]) -> usize {
    if ranges.len() < 2 {
        return 0;
    }
    let avg_chunk_size: f64 =
        ranges.iter().map(|(_, len)| *len as f64).sum::<f64>() / ranges.len() as f64;
    (avg_chunk_size * 0.25) as usize
}

/// Merge adjacent byte ranges for a single URL into fewer, larger HTTP Range requests.
///
/// Ranges are merged when the gap between them is <= `max_gap_size` and the resulting
/// merged range does not exceed [`MAX_MERGED_RANGE_SIZE`].
fn merge_ranges_for_url(
    url: String,
    mut chunks: Vec<(usize, u64, u64)>, // (original_row_index, offset, length)
    max_gap_size: usize,
    segment_id: Option<String>,
    expected_etag: Option<ETag>,
    registration_time: Option<jiff::Timestamp>,
) -> Vec<MergedRangeRequest> {
    if chunks.is_empty() {
        return vec![];
    }

    // Sort by offset
    chunks.sort_by_key(|&(_, offset, _)| offset);
    // Deduplicate ranges with same offset, keeping the first one
    chunks.dedup_by_key(|(_, offset, _)| *offset);

    let mut merged_ranges = Vec::new();
    let (first_row, first_offset, first_length) = chunks[0];
    let mut current_start = first_offset as usize;
    let mut current_end = (first_offset + first_length) as usize;
    let mut chunk_infos = vec![ChunkInMergedRange {
        original_row_index: first_row,
        offset_in_merged: 0,
        length: first_length as usize,
    }];

    for (row_idx, offset, length) in chunks.into_iter().skip(1) {
        let offset = offset as usize;
        let length = length as usize;
        let gap_size = offset.saturating_sub(current_end);

        let new_end = (offset + length).max(current_end);
        let new_merged_size = new_end - current_start;

        let should_merge = gap_size <= max_gap_size && new_merged_size <= MAX_MERGED_RANGE_SIZE;

        if should_merge {
            chunk_infos.push(ChunkInMergedRange {
                original_row_index: row_idx,
                offset_in_merged: offset - current_start,
                length,
            });
            current_end = new_end;
        } else {
            merged_ranges.push(MergedRangeRequest {
                url: url.clone(),
                file_range_start: current_start,
                file_range_end: current_end,
                chunks: chunk_infos,
                segment_id: segment_id.clone(),
                expected_etag: expected_etag.clone(),
                registration_time,
            });

            current_start = offset;
            current_end = offset + length;
            chunk_infos = vec![ChunkInMergedRange {
                original_row_index: row_idx,
                offset_in_merged: 0,
                length,
            }];
        }
    }

    // Don't forget the last range
    merged_ranges.push(MergedRangeRequest {
        url,
        file_range_start: current_start,
        file_range_end: current_end,
        chunks: chunk_infos,
        segment_id,
        expected_etag,
        registration_time,
    });

    merged_ranges
}

/// Calculate adaptive concurrency based on range sizes and total data volume.
///
/// Small ranges are latency-bound (high concurrency helps), large ranges are
/// bandwidth-bound (fewer concurrent requests avoids contention).
fn calculate_adaptive_concurrency(ranges: &[(u64, u64)]) -> usize {
    if ranges.is_empty() {
        return 1;
    }
    let total_range_size: usize = ranges.iter().map(|(_, len)| *len as usize).sum();
    let avg_range_size = total_range_size / ranges.len();

    // Factor 1: range size determines base concurrency
    let base_concurrency = if avg_range_size <= 128 * 1024 {
        130
    } else if avg_range_size <= 2 * 1024 * 1024 {
        90
    } else {
        30
    };

    // Factor 2: memory pressure limiter based on total data
    let memory_limit = if total_range_size <= 50 * 1024 * 1024 {
        base_concurrency
    } else if total_range_size <= 200 * 1024 * 1024 {
        25
    } else {
        8
    };

    base_concurrency.min(memory_limit)
}

/// Decode a single chunk from raw RRD bytes (protobuf-encoded `ArrowMsg`).
#[tracing::instrument(level = "debug", skip_all)]
fn decode_chunk_from_bytes(bytes: &[u8]) -> Result<(Chunk, Option<String>), DirectFetchError> {
    re_tracing::profile_function!();
    use re_log_encoding::Decodable;
    let raw_msg =
        <Option<re_protos::log_msg::v1alpha1::log_msg::Msg> as Decodable>::from_rrd_bytes(bytes)
            .map_err(|err| {
                DirectFetchError::new(format!("Msg::from_rrd_bytes failed: {err}"), false)
            })?
            .ok_or_else(|| DirectFetchError::new("empty msg".to_owned(), false))?;
    let re_protos::log_msg::v1alpha1::log_msg::Msg::ArrowMsg(arrow_msg) = raw_msg else {
        return Err(DirectFetchError::new("invalid msg type".to_owned(), false));
    };

    let segment_id_opt = arrow_msg.store_id.clone().map(|id| id.recording_id);

    use re_log_encoding::ToApplication as _;
    let app_msg = arrow_msg.to_application(()).map_err(|err| {
        DirectFetchError::new(format!("ArrowMsg::to_application() failed: {err}"), false)
    })?;

    let chunk = Chunk::from_record_batch(&app_msg.batch).map_err(|err| {
        DirectFetchError::new(format!("Chunk::from_record_batch failed: {err}"), false)
    })?;

    Ok((chunk, segment_id_opt))
}

/// Fetches chunks for a single request batch using direct URLs and HTTP Range requests.
///
/// Adjacent byte ranges targeting the same URL are merged into larger HTTP Range requests
/// to reduce round-trips. Concurrency is adapted based on range sizes and total data volume.
/// The bytes at those offsets are protobuf-encoded `ArrowMsg` payloads
/// (the 16-byte `MessageHeader` has already been excluded from the manifest offsets).
#[tracing::instrument(
    level = "info",
    skip_all,
    fields(num_chunks, num_merged_requests, concurrency)
)]
async fn fetch_batch_via_direct_urls(
    http_client: &reqwest::Client,
    batch: &RecordBatch,
    stats: &mut TaskFetchStats,
) -> Result<Vec<ChunksWithSegment>, DirectFetchError> {
    fn batch_column<'a, T: arrow::array::Array + 'static>(
        batch: &'a RecordBatch,
        column_name: &'static str,
    ) -> Result<&'a T, DirectFetchError> {
        let column = batch
            .column_by_name(column_name)
            .ok_or_else(|| DirectFetchError::new(format!("missing column {column_name}"), false))?;
        column
            .as_any()
            .downcast_ref::<T>()
            .ok_or_else(|| DirectFetchError::new(format!("invalid column {column_name}"), false))
    }

    // The fetchable URL comes from `direct_url` (presigned `https://`),
    // populated by the server. `chunk_key` carries the canonical source URL
    // (e.g. `s3://`) plus per-source-object metadata (etag, registration_time)
    // used here purely for drift detection — never as the transport URL.
    let chunk_keys: &BinaryArray = batch_column(batch, QueryDatasetResponse::FIELD_CHUNK_KEY)?;
    let direct_urls =
        batch_column::<DictionaryArray<Int32Type>>(batch, QueryDatasetResponse::FIELD_DIRECT_URL)?
            .downcast_dict::<StringArray>()
            .ok_or_else(|| {
                DirectFetchError::new("direct_url dict values must be strings".to_owned(), false)
            })?;
    // Segment IDs are required on QueryDatasetResponse, but treat them as
    // optional here: we use them purely for diagnostic logging on the decode
    // failure path, and a missing column should never break the fetch path.
    let segment_ids: Option<&StringArray> = batch
        .column_by_name(QueryDatasetResponse::FIELD_CHUNK_SEGMENT_ID)
        .and_then(|col| col.as_any().downcast_ref::<StringArray>());

    let num_rows = batch.num_rows();

    // Step 1: Group chunks by URL and collect all ranges for gap/concurrency calculations.
    // ETag and registration_time are per-source-object (per URL), so all rows
    // sharing a URL share both, and we stash them once on first sight.
    struct UrlGroup {
        ranges: Vec<(usize, u64, u64)>,
        segment_id: Option<String>,
        expected_etag: Option<ETag>,
        registration_time: Option<jiff::Timestamp>,
    }
    let mut url_groups: BTreeMap<String, UrlGroup> = BTreeMap::new();
    let mut all_ranges: Vec<(u64, u64)> = Vec::with_capacity(num_rows);

    for i in 0..num_rows {
        if chunk_keys.is_null(i) || direct_urls.is_null(i) {
            return Err(DirectFetchError::new(
                format!("null chunk_key or direct_url at row {i}"),
                false,
            ));
        }
        let chunk_key = ChunkKey::try_from(chunk_keys.value(i)).map_err(|err| {
            DirectFetchError::new(
                format!("failed to decode chunk_key at row {i}: {err}"),
                false,
            )
        })?;
        let rrd_location =
            RrdChunkLocation::try_from(chunk_key.location.as_slice()).map_err(|err| {
                DirectFetchError::new(
                    format!("failed to decode RrdChunkLocation at row {i}: {err}"),
                    false,
                )
            })?;

        let url = direct_urls.value(i).to_owned();
        let offset = rrd_location.offset;
        let length = rrd_location.length;

        url_groups
            .entry(url)
            .or_insert_with(|| UrlGroup {
                ranges: Vec::new(),
                segment_id: segment_ids
                    .filter(|arr| !arr.is_null(i))
                    .map(|arr| arr.value(i).to_owned()),
                expected_etag: chunk_key.etag,
                registration_time: chunk_key.registration_time,
            })
            .ranges
            .push((i, offset, length));
        all_ranges.push((offset, length));
    }

    // Step 2: Merge adjacent ranges per URL.
    let max_gap_size = calculate_optimal_gap_size(&all_ranges);
    let merged_requests: Vec<MergedRangeRequest> = url_groups
        .into_iter()
        .flat_map(|(url, group)| {
            merge_ranges_for_url(
                url,
                group.ranges,
                max_gap_size,
                group.segment_id,
                group.expected_etag,
                group.registration_time,
            )
        })
        .collect();

    // Step 3: Calculate adaptive concurrency from original (un-merged) ranges.
    let concurrency = calculate_adaptive_concurrency(&all_ranges);

    let span = tracing::Span::current();
    span.record("num_chunks", num_rows);
    span.record("num_merged_requests", merged_requests.len());
    span.record("concurrency", concurrency);

    stats.record_direct_ranges(all_ranges.len() as u64, merged_requests.len() as u64);

    re_log::debug!(
        "Range merging: {num_rows} chunks → {} merged requests, concurrency={concurrency}",
        merged_requests.len()
    );

    // Step 4: Fetch merged ranges concurrently and extract individual chunks.
    //
    // Each inner future owns its own `TaskFetchStats` so nothing touches a
    // shared cache line across threads during the retry-heavy hot path. The
    // per-future buffers are merged into the outer task's accumulator below.
    let fetches = merged_requests
        .into_iter()
        .enumerate()
        .map(|(req_idx, request)| {
            let http_client = http_client.clone();
            async move {
                let mut local_stats = TaskFetchStats::default();
                // Range headers are inclusive
                let range_end = request.file_range_end - 1;
                re_log::debug!(
                    "Merged fetch [{req_idx}]: {start}..={range_end} ({} chunks)",
                    request.chunks.len(),
                    start = request.file_range_start,
                );

                // Backoff matching gRPC retry settings: base 100ms, max 3s, 50% jitter.
                let mut backoff_gen = re_backoff::BackoffGenerator::new(
                    Duration::from_millis(100),
                    Duration::from_secs(3),
                )
                .expect("base is less than max");

                let mut last_err: Option<DirectFetchError> = None;
                for attempt in 1..=DIRECT_FETCH_MAX_RETRIES {
                    if last_err.is_some() {
                        let backoff = backoff_gen.gen_next();
                        let jittered = backoff.jittered();
                        re_log::debug!(
                            "Direct fetch [{req_idx}] retry attempt {attempt}/{DIRECT_FETCH_MAX_RETRIES} after {jittered:?}"
                        );
                        if attempt == 2 {
                            // Count this merged request as "needed a retry" on the first retry only.
                            local_stats.record_direct_request_was_retried();
                        }
                        local_stats.record_direct_retry(jittered, attempt as u64);
                        backoff.sleep().await;
                    }

                    let fetch_result =
                        fetch_merged_range(&http_client, &request, range_end).await;

                    match fetch_result {
                        Ok(results) => {
                            if attempt > 1 {
                                re_log::debug!(
                                    "Direct fetch [{req_idx}] succeeded on attempt {attempt}"
                                );
                            }
                            return (Ok(results), local_stats);
                        }
                        Err(err) if err.retryable => {
                            re_log::debug!(
                                "Direct fetch [{req_idx}] failure (attempt {attempt}/{DIRECT_FETCH_MAX_RETRIES}): {err}"
                            );
                            last_err = Some(err);
                        }
                        Err(err) => {
                            re_log::error!(
                                "Non-retryable direct fetch failure on attempt {attempt}: {err}"
                            );
                            return (Err(err), local_stats);
                        }
                    }
                }

                let err = last_err.expect("at least one attempt was made");
                (
                    Err(DirectFetchError::new(
                        format!(
                            "request [{req_idx}] failed after {DIRECT_FETCH_MAX_RETRIES} attempts: {err}"
                        ),
                        false,
                    )),
                    local_stats,
                )
            }
            .instrument(tracing::info_span!(
                "direct_fetch_request",
                req = req_idx,
                bytes = tracing::field::Empty
            ))
        });

    // Fold every inner buffer into the outer task's accumulator before we bail
    // on the first error — we want stats from successful fetches preserved.
    let mut all_chunks: Vec<(usize, (Chunk, Option<String>))> = Vec::new();
    let mut first_err: Option<DirectFetchError> = None;
    async {
        let mut stream = futures::stream::iter(fetches).buffer_unordered(concurrency);
        while let Some((result, local_stats)) = stream.next().await {
            stats.merge_from(local_stats);
            match result {
                Ok(chunks) => all_chunks.extend(chunks),
                Err(err) => {
                    if first_err.is_none() {
                        first_err = Some(err);
                    }
                }
            }
        }
    }
    .instrument(tracing::info_span!("direct_fetch_all"))
    .await;
    if let Some(err) = first_err {
        return Err(err);
    }

    // Step 5: Reassemble in original row order.
    all_chunks.sort_by_key(|(idx, _)| *idx);
    let ordered: Vec<(Chunk, Option<String>)> = all_chunks
        .into_iter()
        .map(|(_, chunk_with_segment)| chunk_with_segment)
        .collect();

    Ok(vec![ordered])
}

type DecodedChunk = (usize, (Chunk, Option<String>));

async fn fetch_merged_range(
    http_client: &reqwest::Client,
    request: &MergedRangeRequest,
    range_end: usize,
) -> Result<Vec<DecodedChunk>, DirectFetchError> {
    let MergedRangeRequest {
        url,
        file_range_start: range_start,
        file_range_end: _,
        chunks,
        segment_id,
        expected_etag,
        registration_time,
    } = request;
    let segment_id = segment_id.as_deref();
    let expected_etag = expected_etag.as_ref();
    let registration_time = *registration_time;

    let mut http_request = http_client
        .get(url)
        .header("Range", format!("bytes={range_start}-{range_end}"));

    // If-Match header to detect manifest drift at the source.
    if let Some(etag) = expected_etag.and_then(ETag::as_if_match) {
        http_request = http_request.header(reqwest::header::IF_MATCH, etag);
    }
    let response = http_request.send().await?;

    if response.status() == reqwest::StatusCode::PRECONDITION_FAILED {
        return Err(DirectFetchError::source_changed(segment_id));
    }

    if !response.status().is_success() {
        return Err(classify_http_status(response.status()));
    }

    // Captured for decode-failure attribution (RR-4549): compared against
    // `expected_etag` if the chunk fails to decode. `last_modified` is
    // logged alongside for diagnostics.
    let returned_etag: Option<ETag> = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(ETag::new);
    let last_modified = response
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    let merged_bytes = response
        .bytes()
        .await
        .map_err(|err| DirectFetchError::new(format!("failed to read body: {err}"), true))?;

    tracing::Span::current().record("bytes", merged_bytes.len());

    // Extract individual chunks from the merged response.
    // Deep copy each chunk to avoid holding the entire merged buffer alive.
    chunks
        .iter()
        .map(|info| {
            let start = info.offset_in_merged;
            let end = start + info.length;
            // Deep copy: prevents holding entire 16MB merged buffer in memory
            let chunk_bytes = merged_bytes.get(start..end).ok_or_else(|| {
                DirectFetchError::new(
                    format!(
                        "merged range shorter than expected: need {end} bytes, got {}",
                        merged_bytes.len()
                    ),
                    false,
                )
            })?;
            decode_chunk_from_bytes(chunk_bytes)
                .map_err(|err| {
                    let logged_url = url_strip_query(url.as_str());
                    let drifted = match (expected_etag, returned_etag.as_ref()) {
                        (Some(want), Some(got)) => !want.matches(got),
                        _ => false,
                    };
                    re_log::error!(
                        segment_id = segment_id.unwrap_or("unknown"),
                        url = logged_url,
                        range_start,
                        range_end,
                        chunk_offset = info.offset_in_merged,
                        chunk_length = info.length,
                        expected_etag = ?expected_etag,
                        actual_etag = ?returned_etag,
                        object_last_modified = ?last_modified,
                        registration_time = ?registration_time,
                        drifted,
                        %err,
                        "failed decoding bytes from direct fetch",
                    );
                    if drifted {
                        DirectFetchError::source_changed(segment_id)
                    } else {
                        err
                    }
                })
                .map(|chunk_with_segment| (info.original_row_index, chunk_with_segment))
        })
        .collect::<Result<Vec<_>, _>>()
}
