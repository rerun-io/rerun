//! Chunk fetching strategies: direct URL (HTTP Range) and gRPC.

use std::collections::BTreeMap;
use std::{error::Error as _, fmt::Write as _};

use arrow::array::{Array as _, DictionaryArray, RecordBatch, StringArray, UInt64Array};
use arrow::datatypes::Int32Type;
use datafusion::common::exec_datafusion_err;
use datafusion::error::DataFusionError;
use futures::StreamExt as _;
use tonic::IntoRequest as _;
use tracing::Instrument as _;

use re_dataframe::external::re_chunk::Chunk;
use re_protos::cloud::v1alpha1::{FetchChunksRequest, QueryDatasetResponse};
use re_redap_client::ApiResult;

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
const DIRECT_FETCH_MAX_RETRIES: usize = 4;

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
}

/// Error from a direct URL fetch attempt. Retried up to [`DIRECT_FETCH_MAX_RETRIES`] times.
#[derive(Debug)]
pub struct DirectFetchError(String);

impl std::fmt::Display for DirectFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
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

/// Fetch a batch of chunks via direct URLs with retries.
///
/// Returns a hard error if all retry attempts are exhausted.
#[tracing::instrument(level = "trace", skip_all)]
pub async fn fetch_batch_direct(
    batch: &RecordBatch,
    http_client: &reqwest::Client,
) -> Result<Vec<ChunksWithSegment>, DataFusionError> {
    #[cfg(not(target_arch = "wasm32"))]
    let byte_size = batch_byte_size(batch);

    // Backoff matching gRPC retry settings: base 100ms, max 3s, 50% jitter.
    let mut backoff_gen = re_backoff::BackoffGenerator::new(
        std::time::Duration::from_millis(100),
        std::time::Duration::from_secs(3),
    )
    .expect("base is less than max");

    let mut last_err: Option<DirectFetchError> = None;
    for attempt in 1..=DIRECT_FETCH_MAX_RETRIES {
        if last_err.is_some() {
            let backoff = backoff_gen.gen_next();
            re_log::debug!(
                "Direct fetch retry attempt {attempt}/{DIRECT_FETCH_MAX_RETRIES} after {:?}",
                backoff.jittered()
            );
            backoff.sleep().await;
        }

        match fetch_batch_via_direct_urls(http_client, batch).await {
            Ok(chunks) => {
                if attempt > 1 {
                    re_log::debug!("Direct fetch succeeded on attempt {attempt}");
                }
                #[cfg(not(target_arch = "wasm32"))]
                metrics::record_direct_success(byte_size);
                return Ok(chunks);
            }
            Err(err) => {
                re_log::debug!(
                    "Direct fetch failure (attempt {attempt}/{DIRECT_FETCH_MAX_RETRIES}): {err}"
                );
                last_err = Some(err);
            }
        }
    }

    let err = last_err.expect("at least one attempt was made");
    let reason = classify_failure_reason(&err);
    #[cfg(not(target_arch = "wasm32"))]
    metrics::record_direct_failure(reason);
    Err(exec_datafusion_err!(
        "Direct fetch failed after {DIRECT_FETCH_MAX_RETRIES} attempts: {err}"
    ))
}

/// Classify a `DirectFetchError` into a bounded metric reason tag.
fn classify_failure_reason(err: &DirectFetchError) -> &'static str {
    let msg = &err.0;
    if msg.contains("timed out") || msg.contains("Timeout") {
        "timeout"
    } else if msg.contains("status 4") {
        "http_4xx"
    } else if msg.contains("status 5") {
        "http_5xx"
    } else if msg.contains("connection") || msg.contains("dns") || msg.contains("connect") {
        "connection"
    } else if msg.contains("decode")
        || msg.contains("from_rrd_bytes")
        || msg.contains("from_record_batch")
    {
        "decode"
    } else {
        "other"
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

        let response = client
            .fetch_chunks(fetch_chunks_request.into_request())
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
    DirectFetchError(format!("HTTP request returned status {status}"))
}

impl From<reqwest::Error> for DirectFetchError {
    fn from(err: reqwest::Error) -> Self {
        let mut msg = match err.status() {
            Some(status) => {
                format!("HTTP request failed with status {status}: {err}")
            }
            None => format!("HTTP request failed: {err}"),
        };

        if let Some(source) = err.source() {
            write!(msg, " ({source})").expect("Can append");
        }

        Self(msg)
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
fn decode_chunk_from_bytes(bytes: &[u8]) -> Result<(Chunk, Option<String>), DirectFetchError> {
    use re_log_encoding::Decodable;
    let raw_msg =
        <Option<re_protos::log_msg::v1alpha1::log_msg::Msg> as Decodable>::from_rrd_bytes(bytes)
            .map_err(|err| DirectFetchError(format!("Msg::from_rrd_bytes failed: {err}")))?
            .ok_or_else(|| DirectFetchError("empty msg".to_owned()))?;
    let re_protos::log_msg::v1alpha1::log_msg::Msg::ArrowMsg(arrow_msg) = raw_msg else {
        return Err(DirectFetchError("invalid msg type".to_owned()));
    };

    let segment_id_opt = arrow_msg.store_id.clone().map(|id| id.recording_id);

    use re_log_encoding::ToApplication as _;
    let app_msg = arrow_msg
        .to_application(())
        .map_err(|err| DirectFetchError(format!("ArrowMsg::to_application() failed: {err}")))?;

    let chunk = Chunk::from_record_batch(&app_msg.batch)
        .map_err(|err| DirectFetchError(format!("Chunk::from_record_batch failed: {err}")))?;

    Ok((chunk, segment_id_opt))
}

/// Fetches chunks for a single request batch using direct URLs and HTTP Range requests.
///
/// Adjacent byte ranges targeting the same URL are merged into larger HTTP Range requests
/// to reduce round-trips. Concurrency is adapted based on range sizes and total data volume.
/// The bytes at those offsets are protobuf-encoded `ArrowMsg` payloads
/// (the 16-byte `MessageHeader` has already been excluded from the manifest offsets).
#[tracing::instrument(level = "trace", skip_all)]
async fn fetch_batch_via_direct_urls(
    http_client: &reqwest::Client,
    batch: &RecordBatch,
) -> Result<Vec<ChunksWithSegment>, DirectFetchError> {
    fn batch_column<'a, T: arrow::array::Array + 'static>(
        batch: &'a RecordBatch,
        column_name: &'static str,
    ) -> Result<&'a T, DirectFetchError> {
        let column = batch
            .column_by_name(column_name)
            .ok_or_else(|| DirectFetchError(format!("missing column {column_name}")))?;
        column
            .as_any()
            .downcast_ref::<T>()
            .ok_or_else(|| DirectFetchError(format!("invalid column {column_name}")))
    }

    let byte_offsets: &UInt64Array =
        batch_column(batch, QueryDatasetResponse::FIELD_CHUNK_BYTE_OFFSET)?;
    let byte_lengths: &UInt64Array =
        batch_column(batch, QueryDatasetResponse::FIELD_CHUNK_BYTE_LENGTH)?;
    let direct_urls: &DictionaryArray<Int32Type> =
        batch_column(batch, QueryDatasetResponse::FIELD_DIRECT_URL)?;

    let num_rows = batch.num_rows();

    // Step 1: Group chunks by URL and collect all ranges for gap/concurrency calculations.
    let mut url_groups: BTreeMap<String, Vec<(usize, u64, u64)>> = BTreeMap::new();
    let mut all_ranges: Vec<(u64, u64)> = Vec::with_capacity(num_rows);

    let url_values = direct_urls
        .values()
        .as_any()
        .downcast_ref::<StringArray>()
        .expect("direct_url dictionary values must be strings");

    for i in 0..num_rows {
        let offset = byte_offsets.value(i);
        let length = byte_lengths.value(i);

        re_log::debug_assert!(
            !direct_urls.is_null(i),
            "split_batch_by_direct_url should have filtered null URLs"
        );
        if direct_urls.is_null(i) {
            return Err(DirectFetchError(format!(
                "no direct URL for chunk at row {i}"
            )));
        }
        let key = direct_urls.keys().value(i);
        let url = url_values.value(key as usize).to_owned();

        url_groups.entry(url).or_default().push((i, offset, length));
        all_ranges.push((offset, length));
    }

    // Step 2: Merge adjacent ranges per URL.
    let max_gap_size = calculate_optimal_gap_size(&all_ranges);
    let merged_requests: Vec<MergedRangeRequest> = url_groups
        .into_iter()
        .flat_map(|(url, chunks)| merge_ranges_for_url(url, chunks, max_gap_size))
        .collect();

    // Step 3: Calculate adaptive concurrency from original (un-merged) ranges.
    let concurrency = calculate_adaptive_concurrency(&all_ranges);
    re_log::debug!(
        "Range merging: {num_rows} chunks → {} merged requests, concurrency={concurrency}",
        merged_requests.len()
    );

    // Step 4: Fetch merged ranges concurrently and extract individual chunks.
    let fetches = merged_requests
        .into_iter()
        .enumerate()
        .map(|(req_idx, request)| {
            let MergedRangeRequest {
                url,
                file_range_start,
                file_range_end,
                chunks,
            } = request;

            let http_client = http_client.clone();
            async move {
                // Range headers are inclusive
                let range_end = file_range_end - 1;
                re_log::debug!(
                    "Merged fetch [{req_idx}]: {file_range_start}..={range_end} ({} chunks)",
                    chunks.len()
                );

                let response = http_client
                    .get(url)
                    .header("Range", format!("bytes={file_range_start}-{range_end}"))
                    .send()
                    .await?;

                if !response.status().is_success() {
                    return Err(classify_http_status(response.status()));
                }

                let merged_bytes = response
                    .bytes()
                    .await
                    .map_err(|err| DirectFetchError(format!("failed to read body: {err}")))?;

                // Extract individual chunks from the merged response.
                // Deep copy each chunk to avoid holding the entire merged buffer alive.
                let chunk_results = chunks
                    .into_iter()
                    .map(|info| {
                        let start = info.offset_in_merged;
                        let end = start + info.length;
                        // Deep copy: prevents holding entire 16MB merged buffer in memory
                        let chunk_bytes = merged_bytes.get(start..end).ok_or_else(|| {
                            DirectFetchError(format!(
                                "merged range shorter than expected: need {end} bytes, got {}",
                                merged_bytes.len()
                            ))
                        })?;
                        decode_chunk_from_bytes(chunk_bytes)
                            .map(|chunk_with_segment| (info.original_row_index, chunk_with_segment))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(chunk_results)
            }
            .instrument(tracing::debug_span!("merged_fetch", req = req_idx))
        });

    let results: Vec<Result<Vec<_>, DirectFetchError>> = futures::stream::iter(fetches)
        .buffer_unordered(concurrency)
        .collect()
        .instrument(tracing::trace_span!("url_direct_fetch"))
        .await;

    // Any merged range failure fails the whole batch.
    let mut all_chunks: Vec<(usize, (Chunk, Option<String>))> = Vec::new();
    for result in results {
        all_chunks.extend(result?);
    }

    // Step 5: Reassemble in original row order.
    all_chunks.sort_by_key(|(idx, _)| *idx);
    let ordered: Vec<(Chunk, Option<String>)> = all_chunks
        .into_iter()
        .map(|(_, chunk_with_segment)| chunk_with_segment)
        .collect();

    Ok(vec![ordered])
}
