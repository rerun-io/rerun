//! Sanity-check that [`build_metrics_set_for_explain`] populates the
//! `MetricsSet` with the values from [`QueryInfo`] and the runtime
//! atomics on [`QueryMetrics`]. This is the contract that backs
//! `EXPLAIN ANALYZE` output for `SegmentStreamExec`.

use super::*;

fn dummy_query_info(
    filters_pushed_down: usize,
    filters_applied_client_side: usize,
    entity_path_narrowing_applied: bool,
) -> QueryInfo {
    QueryInfo {
        dataset_id: "ds-test".to_owned(),
        query_chunks: 7,
        query_segments: 3,
        query_layers: 2,
        query_columns: 11,
        query_entities: 5,
        query_bytes: 12_345,
        target_partitions: 8,
        query_chunks_per_segment_min: 1,
        query_chunks_per_segment_max: 4,
        query_chunks_per_segment_mean: 2.5,
        query_type: QueryType::LatestAt,
        primary_index_name: Some("log_time".to_owned()),
        time_to_first_chunk_info: Some(Duration::from_millis(2)),
        trace_id: None,
        filters_pushed_down,
        filters_applied_client_side,
        entity_path_narrowing_applied,
        filters_total: 0,
        filters_signatures: String::new(),
        filters_signatures_exact: String::new(),
        filters_signatures_inexact: String::new(),
        filters_signatures_unsupported: String::new(),
    }
}

/// Helper to look up the value of a single `global_counter`-style metric by name.
fn metric_value_by_name(set: &MetricsSet, name: &str) -> Option<usize> {
    set.iter()
        .find(|m| m.value().name() == name)
        .map(|m| m.value().as_usize())
}

#[test]
fn emits_chunk_segment_byte_counts() {
    let metrics = QueryMetrics::new(dummy_query_info(0, 0, false));
    let set = build_metrics_set_for_explain(&metrics, 1, None);

    assert_eq!(metric_value_by_name(&set, "query_chunks"), Some(7));
    assert_eq!(metric_value_by_name(&set, "query_segments"), Some(3));
    assert_eq!(metric_value_by_name(&set, "query_layers"), Some(2));
    assert_eq!(metric_value_by_name(&set, "query_columns"), Some(11));
    assert_eq!(metric_value_by_name(&set, "query_entities"), Some(5));
    assert_eq!(metric_value_by_name(&set, "query_bytes"), Some(12_345));
    assert_eq!(
        metric_value_by_name(&set, "query_chunks_per_segment_max"),
        Some(4),
    );
    assert_eq!(
        metric_value_by_name(&set, "time_to_first_chunk_info_us"),
        Some(2_000),
    );
}

#[test]
fn emits_filter_pushdown_counters() {
    let metrics = QueryMetrics::new(dummy_query_info(2, 1, false));
    let set = build_metrics_set_for_explain(&metrics, 1, None);

    assert_eq!(metric_value_by_name(&set, "filters_pushed_down"), Some(2));
    assert_eq!(
        metric_value_by_name(&set, "filters_applied_client_side"),
        Some(1),
    );
    // Boolean: only emitted when true. With `false` here the metric is absent.
    assert_eq!(
        metric_value_by_name(&set, "entity_path_narrowing_applied"),
        None,
    );
}

#[test]
fn emits_entity_path_narrowing_when_applied() {
    let metrics = QueryMetrics::new(dummy_query_info(0, 0, true));
    let set = build_metrics_set_for_explain(&metrics, 1, None);

    assert_eq!(
        metric_value_by_name(&set, "entity_path_narrowing_applied"),
        Some(1),
    );
}

#[test]
fn emits_runtime_counters_and_partition_count() {
    let metrics = QueryMetrics::new(dummy_query_info(0, 0, false));
    // Simulate two partitions each contributing to the shared atomics.
    metrics.fetch_grpc_bytes.fetch_add(1_000, Ordering::Relaxed);
    metrics.fetch_grpc_bytes.fetch_add(2_500, Ordering::Relaxed);
    metrics
        .fetch_direct_max_attempt
        .fetch_max(3, Ordering::Relaxed);
    metrics
        .fetch_direct_max_attempt
        .fetch_max(5, Ordering::Relaxed);
    metrics
        .fetch_direct_max_attempt
        .fetch_max(2, Ordering::Relaxed);
    metrics
        .planned_fetch_batches
        .fetch_add(16, Ordering::Relaxed);
    metrics
        .planned_segment_waves
        .fetch_add(1_332, Ordering::Relaxed);
    metrics
        .segment_admission_limit
        .fetch_max(3, Ordering::Relaxed);
    metrics.segment_admission_source.store(
        crate::metrics_capture::SegmentAdmissionSource::MetricsOnly as u64,
        Ordering::Relaxed,
    );
    metrics.segment_admission_candidate_reason.store(
        crate::metrics_capture::SegmentAdmissionCandidateReason::Eligible as u64,
        Ordering::Relaxed,
    );
    metrics
        .max_segments_per_fetch_batch
        .fetch_max(2, Ordering::Relaxed);
    metrics
        .max_segments_per_wave
        .fetch_max(3, Ordering::Relaxed);
    metrics.peak_active_segments.fetch_max(3, Ordering::Relaxed);
    metrics
        .pipeline_budget_bytes
        .store(4 * 1024 * 1024 * 1024, Ordering::Relaxed);
    metrics
        .pipeline_peak_decoded_bytes
        .fetch_max(96 * 1024 * 1024, Ordering::Relaxed);
    metrics.pipeline_byte_waits.fetch_add(4, Ordering::Relaxed);
    metrics
        .segment_admission_waits
        .fetch_add(20, Ordering::Relaxed);
    metrics
        .pipeline_stall_breaker_activations
        .fetch_add(1, Ordering::Relaxed);
    metrics.delivered_rows.fetch_add(150, Ordering::Relaxed);
    metrics.delivered_bytes.fetch_add(6_000, Ordering::Relaxed);
    metrics.decode_time_us.fetch_add(2_000, Ordering::Relaxed);
    metrics
        .peak_inflight_fetches
        .fetch_max(7, Ordering::Relaxed);

    let set = build_metrics_set_for_explain(&metrics, 4, None);

    assert_eq!(metric_value_by_name(&set, "delivered_rows"), Some(150));
    assert_eq!(metric_value_by_name(&set, "delivered_bytes"), Some(6_000));
    assert_eq!(
        metric_value_by_name(&set, "decode_duration_us"),
        Some(2_000)
    );
    assert_eq!(metric_value_by_name(&set, "peak_inflight_fetches"), Some(7));
    assert_eq!(metric_value_by_name(&set, "fetch_grpc_bytes"), Some(3_500));
    // True cross-partition max, not a sum.
    assert_eq!(
        metric_value_by_name(&set, "fetch_direct_max_attempt"),
        Some(5),
    );
    assert_eq!(metric_value_by_name(&set, "num_partitions"), Some(4));
    assert_eq!(
        metric_value_by_name(&set, "planned_fetch_batches"),
        Some(16)
    );
    assert_eq!(
        metric_value_by_name(&set, "planned_segment_waves"),
        Some(1_332)
    );
    assert_eq!(
        metric_value_by_name(&set, "segment_admission_limit"),
        Some(3)
    );
    assert_eq!(
        metric_value_by_name(&set, "segment_admission_source_code"),
        Some(crate::metrics_capture::SegmentAdmissionSource::MetricsOnly as usize)
    );
    assert_eq!(
        metric_value_by_name(&set, "segment_admission_candidate_reason_code"),
        Some(crate::metrics_capture::SegmentAdmissionCandidateReason::Eligible as usize)
    );
    assert_eq!(
        metric_value_by_name(&set, "max_segments_per_fetch_batch"),
        Some(2)
    );
    assert_eq!(metric_value_by_name(&set, "max_segments_per_wave"), Some(3));
    assert_eq!(metric_value_by_name(&set, "peak_active_segments"), Some(3));
    assert_eq!(
        metric_value_by_name(&set, "pipeline_budget_bytes"),
        Some(4 * 1024 * 1024 * 1024)
    );
    assert_eq!(
        metric_value_by_name(&set, "pipeline_peak_decoded_bytes"),
        Some(96 * 1024 * 1024)
    );
    assert_eq!(metric_value_by_name(&set, "pipeline_byte_waits"), Some(4));
    assert_eq!(
        metric_value_by_name(&set, "segment_admission_waits"),
        Some(20)
    );
    assert_eq!(
        metric_value_by_name(&set, "pipeline_stall_breaker_activations"),
        Some(1)
    );
}
