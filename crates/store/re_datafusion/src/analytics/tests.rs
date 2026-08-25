use std::collections::HashSet;

use super::*;

fn dummy_query_info() -> QueryInfo {
    QueryInfo {
        dataset_id: "ds-123".to_owned(),
        query_chunks: 42,
        query_segments: 5,
        query_layers: 2,
        query_columns: 7,
        query_entities: 3,
        query_bytes: 1234,
        target_partitions: 8,
        query_chunks_per_segment_min: 4,
        query_chunks_per_segment_max: 12,
        query_chunks_per_segment_mean: 8.4,
        query_type: QueryType::LatestAt,
        primary_index_name: None,
        time_to_first_chunk_info: None,
        trace_id: None,
        filters_pushed_down: 0,
        filters_applied_client_side: 0,
        entity_path_narrowing_applied: false,
        filters_total: 0,
        filters_signatures: String::new(),
        filters_signatures_exact: String::new(),
        filters_signatures_inexact: String::new(),
        filters_signatures_unsupported: String::new(),
    }
}

#[test]
fn assign_span_identity_uses_correlated_trace_id() {
    let trace_id = opentelemetry::TraceId::from_bytes([7; 16]);
    let mut span = Span::default();

    assign_span_identity(&mut span, Some(trace_id)).unwrap();

    assert_eq!(span.trace_id, trace_id.to_bytes());
    assert_eq!(span.span_id.len(), 8);
    assert!(span.span_id.iter().any(|byte| *byte != 0));
}

#[test]
fn assign_span_identity_generates_trace_id_without_correlation() {
    let mut span = Span::default();

    assign_span_identity(&mut span, None).unwrap();

    assert_eq!(span.trace_id.len(), 16);
    assert!(span.trace_id.iter().any(|byte| *byte != 0));
    assert_eq!(span.span_id.len(), 8);
    assert!(span.span_id.iter().any(|byte| *byte != 0));
}

fn attribute_keys(span: &Span) -> HashSet<&str> {
    let keys: HashSet<_> = span.attributes.iter().map(|kv| kv.key.as_str()).collect();
    re_log::debug_assert_eq!(
        keys.len(),
        span.attributes.len(),
        "span contains duplicate attribute keys"
    );
    keys
}

fn find_int(span: &Span, key: &str) -> Option<i64> {
    span.attributes
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
            Value::IntValue(i) => Some(*i),
            _ => None,
        })
}

fn find_string<'a>(span: &'a Span, key: &str) -> Option<&'a str> {
    span.attributes
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
            Value::StringValue(s) => Some(s.as_str()),
            _ => None,
        })
}

fn find_double(span: &Span, key: &str) -> Option<f64> {
    span.attributes
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
            Value::DoubleValue(d) => Some(*d),
            _ => None,
        })
}

fn find_bool(span: &Span, key: &str) -> Option<bool> {
    span.attributes
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
            Value::BoolValue(b) => Some(*b),
            _ => None,
        })
}

/// Required attributes that must always be emitted, regardless of query state.
/// Adding/removing one of these is a breaking change for the analytics pipeline.
const REQUIRED_KEYS: &[&str] = &[
    "dataset_id",
    "query_chunks",
    "query_segments",
    "query_layers",
    "query_columns",
    "query_entities",
    "query_bytes",
    "query_chunks_per_segment_min",
    "query_chunks_per_segment_max",
    "query_chunks_per_segment_mean",
    "query_type",
    "total_duration_us",
    "is_success",
    "fetch_grpc_requests",
    "fetch_grpc_bytes",
    "fetch_direct_requests",
    "fetch_direct_bytes",
    "fetch_direct_retries",
    "fetch_direct_requests_retried",
    "fetch_direct_retry_sleep_us",
    "fetch_direct_max_attempt",
    "fetch_direct_original_ranges",
    "fetch_direct_merged_ranges",
    "planned_fetch_batches",
    "planned_segment_waves",
    "segment_admission_limit",
    "segment_admission_candidate_limit",
    "segment_admission_source",
    "segment_admission_candidate_reason",
    "segment_admission_adaptive_enabled",
    "segment_admission_profile_segment_count",
    "segment_admission_profile_complete",
    "segment_admission_p95_segment_bytes",
    "segment_admission_max_segment_bytes",
    "segment_admission_largest_window_bytes",
    "max_segments_per_fetch_batch",
    "max_segments_per_wave",
    "peak_active_segments",
    "pipeline_budget_bytes",
    "pipeline_peak_decoded_bytes",
    "pipeline_byte_waits",
    "segment_admission_waits",
    "pipeline_stall_breaker_activations",
    "filters_pushed_down",
    "filters_applied_client_side",
    "entity_path_narrowing_applied",
    "query_target_partitions",
    "delivered_rows",
    "delivered_bytes",
    "decode_duration_us",
    "peak_inflight_fetches",
];

/// Build a fresh `QuerySnapshot` mirroring `dummy_query_info()`, with no
/// execution data — analogous to the pre-refactor `TaskFetchStats::default`
/// fixture.
fn snapshot_from_info(query_info: QueryInfo) -> QuerySnapshot {
    QuerySnapshot {
        query_info,
        total_duration: Duration::ZERO,
        time_to_first_chunk: None,
        error_kind: None,
        direct_terminal_reason: None,
        fetch_grpc_requests: 0,
        fetch_grpc_bytes: 0,
        fetch_direct_requests: 0,
        fetch_direct_bytes: 0,
        fetch_direct_retries: 0,
        fetch_direct_requests_retried: 0,
        fetch_direct_retry_sleep: Duration::ZERO,
        fetch_direct_max_attempt: 0,
        fetch_direct_original_ranges: 0,
        fetch_direct_merged_ranges: 0,
        planned_fetch_batches: 0,
        planned_segment_waves: 0,
        segment_admission_limit: 0,
        segment_admission_candidate_limit: 0,
        segment_admission_source: "metrics_only",
        segment_admission_candidate_reason: "eligible",
        segment_admission_adaptive_enabled: false,
        segment_admission_profile_segment_count: 0,
        segment_admission_profile_complete: false,
        segment_admission_p95_segment_bytes: 0,
        segment_admission_max_segment_bytes: 0,
        segment_admission_largest_window_bytes: 0,
        max_segments_per_fetch_batch: 0,
        max_segments_per_wave: 0,
        peak_active_segments: 0,
        pipeline_budget_bytes: 0,
        pipeline_peak_decoded_bytes: 0,
        pipeline_byte_waits: 0,
        segment_admission_waits: 0,
        pipeline_stall_breaker_activations: 0,
        delivered_rows: 0,
        delivered_bytes: 0,
        decode_duration: Duration::ZERO,
        peak_inflight_fetches: 0,
    }
}

#[test]
fn task_fetch_stats_accumulates_and_flushes_decode_time() {
    let metrics = QueryMetrics::new(dummy_query_info());
    let mut a = TaskFetchStats::default();
    a.record_decode(Duration::from_micros(300));
    let mut b = TaskFetchStats::default();
    b.record_decode(Duration::from_micros(700));
    a.merge_from(b);
    a.flush_into(&metrics);
    assert_eq!(metrics.decode_time_us.load(Ordering::Relaxed), 1_000);

    // A default (idle) task must not touch the shared counter.
    let before = metrics.decode_time_us.load(Ordering::Relaxed);
    TaskFetchStats::default().flush_into(&metrics);
    assert_eq!(metrics.decode_time_us.load(Ordering::Relaxed), before);
}

#[test]
fn build_query_span_minimal_emits_only_required_attributes() {
    let qi = dummy_query_info();
    let mut snap = snapshot_from_info(qi);
    snap.total_duration = Duration::from_micros(500);

    let span = build_query_span(
        &snap,
        SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
    );

    // Span shape
    assert_eq!(span.name, "cloud_query_dataset");
    assert_eq!(span.kind, i32::from(SpanKind::Client));
    assert!(span.links.is_empty());

    // Attribute key set is exactly the required keys — no optional keys leaked.
    let expected: HashSet<&str> = REQUIRED_KEYS.iter().copied().collect();
    let actual = attribute_keys(&span);
    assert_eq!(
        actual,
        expected,
        "extra/missing attribute keys: {:?}",
        actual.symmetric_difference(&expected).collect::<Vec<_>>()
    );

    // Spot-check a few values.
    assert_eq!(find_string(&span, "dataset_id"), Some("ds-123"));
    assert_eq!(find_int(&span, "query_chunks"), Some(42));
    assert_eq!(find_int(&span, "query_chunks_per_segment_min"), Some(4));
    assert_eq!(find_int(&span, "query_chunks_per_segment_max"), Some(12));
    assert_eq!(
        find_double(&span, "query_chunks_per_segment_mean"),
        Some(f64::from(8.4_f32))
    );
    assert_eq!(find_string(&span, "query_type"), Some("latest_at"));
    assert_eq!(find_int(&span, "total_duration_us"), Some(500));
    assert_eq!(find_bool(&span, "is_success"), Some(true));
}

#[test]
fn build_query_span_records_fetch_stats() {
    let qi = dummy_query_info();
    let mut snap = snapshot_from_info(qi);
    snap.total_duration = Duration::from_millis(1);
    snap.fetch_grpc_requests = 2;
    snap.fetch_grpc_bytes = 5_000;
    snap.fetch_direct_requests = 1;
    snap.fetch_direct_bytes = 10_000;
    snap.fetch_direct_retries = 2;
    snap.fetch_direct_requests_retried = 1;
    snap.fetch_direct_retry_sleep = Duration::from_millis(12);
    snap.fetch_direct_max_attempt = 3;
    snap.fetch_direct_original_ranges = 8;
    snap.fetch_direct_merged_ranges = 4;
    snap.planned_fetch_batches = 16;
    snap.planned_segment_waves = 1_332;
    snap.segment_admission_limit = 3;
    snap.segment_admission_candidate_limit = 16;
    snap.segment_admission_source = "metrics_only";
    snap.segment_admission_candidate_reason = "eligible";
    snap.segment_admission_adaptive_enabled = true;
    snap.segment_admission_profile_segment_count = 32;
    snap.segment_admission_profile_complete = true;
    snap.segment_admission_p95_segment_bytes = 1024;
    snap.segment_admission_max_segment_bytes = 2048;
    snap.segment_admission_largest_window_bytes = 16_384;
    snap.max_segments_per_fetch_batch = 3;
    snap.max_segments_per_wave = 3;
    snap.peak_active_segments = 3;
    snap.pipeline_budget_bytes = 4 * 1024 * 1024 * 1024;
    snap.pipeline_peak_decoded_bytes = 96 * 1024 * 1024;
    snap.pipeline_byte_waits = 2;
    snap.segment_admission_waits = 1_329;
    snap.pipeline_stall_breaker_activations = 1;
    snap.delivered_rows = 200;
    snap.delivered_bytes = 8_192;
    snap.decode_duration = Duration::from_millis(7);
    snap.peak_inflight_fetches = 11;

    let span = build_query_span(
        &snap,
        SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
    );

    // `dummy_query_info` sets `target_partitions = 8`.
    assert_eq!(find_int(&span, "query_target_partitions"), Some(8));
    assert_eq!(find_int(&span, "delivered_rows"), Some(200));
    assert_eq!(find_int(&span, "delivered_bytes"), Some(8_192));
    assert_eq!(find_int(&span, "decode_duration_us"), Some(7_000));
    assert_eq!(find_int(&span, "peak_inflight_fetches"), Some(11));
    assert_eq!(find_int(&span, "fetch_grpc_requests"), Some(2));
    assert_eq!(find_int(&span, "fetch_grpc_bytes"), Some(5_000));
    assert_eq!(find_int(&span, "fetch_direct_requests"), Some(1));
    assert_eq!(find_int(&span, "fetch_direct_bytes"), Some(10_000));
    assert_eq!(find_int(&span, "fetch_direct_retries"), Some(2));
    assert_eq!(find_int(&span, "fetch_direct_requests_retried"), Some(1));
    assert_eq!(find_int(&span, "fetch_direct_retry_sleep_us"), Some(12_000));
    assert_eq!(find_int(&span, "fetch_direct_max_attempt"), Some(3));
    assert_eq!(find_int(&span, "fetch_direct_original_ranges"), Some(8));
    assert_eq!(find_int(&span, "fetch_direct_merged_ranges"), Some(4));
    assert_eq!(find_int(&span, "planned_fetch_batches"), Some(16));
    assert_eq!(find_int(&span, "planned_segment_waves"), Some(1_332));
    assert_eq!(find_int(&span, "segment_admission_limit"), Some(3));
    assert_eq!(
        find_int(&span, "segment_admission_candidate_limit"),
        Some(16)
    );
    assert_eq!(
        find_string(&span, "segment_admission_source"),
        Some("metrics_only")
    );
    assert_eq!(
        find_string(&span, "segment_admission_candidate_reason"),
        Some("eligible")
    );
    assert_eq!(
        find_bool(&span, "segment_admission_adaptive_enabled"),
        Some(true)
    );
    assert_eq!(
        find_int(&span, "segment_admission_profile_segment_count"),
        Some(32)
    );
    assert_eq!(
        find_bool(&span, "segment_admission_profile_complete"),
        Some(true)
    );
    assert_eq!(
        find_int(&span, "segment_admission_p95_segment_bytes"),
        Some(1024)
    );
    assert_eq!(
        find_int(&span, "segment_admission_max_segment_bytes"),
        Some(2048)
    );
    assert_eq!(
        find_int(&span, "segment_admission_largest_window_bytes"),
        Some(16_384)
    );
    assert_eq!(find_int(&span, "max_segments_per_fetch_batch"), Some(3));
    assert_eq!(find_int(&span, "max_segments_per_wave"), Some(3));
    assert_eq!(find_int(&span, "peak_active_segments"), Some(3));
    assert_eq!(
        find_int(&span, "pipeline_budget_bytes"),
        Some(4 * 1024 * 1024 * 1024)
    );
    assert_eq!(
        find_int(&span, "pipeline_peak_decoded_bytes"),
        Some(96 * 1024 * 1024)
    );
    assert_eq!(find_int(&span, "pipeline_byte_waits"), Some(2));
    assert_eq!(find_int(&span, "segment_admission_waits"), Some(1_329));
    assert_eq!(
        find_int(&span, "pipeline_stall_breaker_activations"),
        Some(1)
    );
}

#[test]
fn build_query_span_emits_all_optional_attributes_when_present() {
    let trace_id = opentelemetry::TraceId::from_bytes([7u8; 16]);
    let mut qi = dummy_query_info();
    qi.primary_index_name = Some("log_time".to_owned());
    qi.time_to_first_chunk_info = Some(Duration::from_micros(123));
    qi.trace_id = Some(trace_id);

    let mut snap = snapshot_from_info(qi);
    snap.total_duration = Duration::from_micros(999);
    snap.time_to_first_chunk = Some(Duration::from_micros(456));
    snap.direct_terminal_reason = Some(DirectFetchFailureReason::Http5xx);
    snap.error_kind = Some(QueryErrorKind::DirectFetch.as_str());

    let span = build_query_span(
        &snap,
        SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
    );

    // Optional keys must all be present.
    let optional = [
        "primary_index_name",
        "time_to_first_chunk_info_us",
        "time_to_first_chunk_us",
        "fetch_direct_terminal_reason",
        "error_kind",
    ];
    let keys = attribute_keys(&span);
    for k in optional {
        assert!(keys.contains(k), "missing optional attribute: {k}");
    }

    // is_success flips to false when error_kind is set.
    assert_eq!(find_bool(&span, "is_success"), Some(false));

    assert_eq!(find_string(&span, "primary_index_name"), Some("log_time"));
    assert_eq!(find_int(&span, "time_to_first_chunk_info_us"), Some(123));
    assert_eq!(find_int(&span, "time_to_first_chunk_us"), Some(456));
    assert_eq!(
        find_string(&span, "fetch_direct_terminal_reason"),
        Some("http_5xx")
    );
    assert_eq!(find_string(&span, "error_kind"), Some("direct_fetch"));
    assert!(span.links.is_empty());
}

/// Confirms the planning-phase metrics that `EXPLAIN ANALYZE` reads are
/// all surfaced by [`build_metrics_set_for_explain`]. Regression check
/// that `_min` shows up — prior to the alignment fix only `_max` was
/// surfaced.
#[test]
fn explain_metrics_set_includes_chunks_per_segment_min_and_max() {
    let metrics = QueryMetrics::new(dummy_query_info());
    let set = build_metrics_set_for_explain(&metrics, 1, None);

    let aggregated = set.aggregate_by_name();
    let names: std::collections::HashSet<_> = aggregated
        .iter()
        .filter_map(|m| match m.value() {
            datafusion::physical_plan::metrics::MetricValue::Count { name, .. } => {
                Some(name.as_ref().to_owned())
            }
            _ => None,
        })
        .collect();

    assert!(
        names.contains("query_chunks_per_segment_min"),
        "expected query_chunks_per_segment_min in explain metrics: {names:?}"
    );
    assert!(
        names.contains("query_chunks_per_segment_max"),
        "expected query_chunks_per_segment_max in explain metrics: {names:?}"
    );
}

#[test]
fn build_query_span_uses_wall_clock_range() {
    let qi = dummy_query_info();
    let snap = snapshot_from_info(qi);
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
    let end = SystemTime::UNIX_EPOCH + Duration::from_millis(2_500);

    let span = build_query_span(&snap, start..end);

    assert_eq!(span.start_time_unix_nano, 2_000_000_000);
    assert_eq!(span.end_time_unix_nano, 2_500_000_000);
}
