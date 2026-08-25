use std::assert_matches;
use std::collections::HashSet;

use re_protos::cloud::v1alpha1::ext::{LanceTable, ProviderDetails, SystemTable};

use super::*;

fn lance_provider_details() -> ProviderDetails {
    // Construct via the protobuf type so we don't need a direct `url`
    // dependency in this crate just for tests.
    let proto = re_protos::cloud::v1alpha1::LanceTable {
        table_url: "s3://bucket/path".to_owned(),
    };
    ProviderDetails::LanceTable(LanceTable::try_from(proto).unwrap())
}

// ---- helpers ----

fn dummy_table_query_info() -> TableQueryInfo {
    TableQueryInfo {
        table_id: "tbl-42".to_owned(),
        table_kind: TableKind::Lance,
        caller: TableQueryCaller::BrowserDetailView,
        schema_total_columns: 12,
        projected_columns: 5,
        has_limit: false,
        limit_value: None,
        time_range: SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        filters_total: 0,
        filters_signatures: String::new(),
    }
}

fn empty_stats() -> TableScanStatsSnapshot {
    TableScanStatsSnapshot::default()
}

fn attribute_keys(span: &Span) -> HashSet<&str> {
    let keys: HashSet<_> = span.attributes.iter().map(|kv| kv.key.as_str()).collect();
    assert_eq!(
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

fn find_bool(span: &Span, key: &str) -> Option<bool> {
    span.attributes
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
            Value::BoolValue(b) => Some(*b),
            _ => None,
        })
}

/// Required attributes that must always be emitted, regardless of scan
/// outcome. Adding/removing one of these is a breaking change for the
/// analytics pipeline (`PostHog` dashboards, server-side enrichment, etc.).
const REQUIRED_KEYS: &[&str] = &[
    "table_id",
    "table_kind",
    "caller",
    "schema_total_columns",
    "projected_columns",
    "has_limit",
    "is_success",
    "total_duration_us",
    "fetch_grpc_requests",
    "num_record_batches",
    "rows_returned",
    "bytes_returned",
];

// ---- builder shape ----

#[test]
fn build_table_query_span_minimal_emits_only_required_attributes() {
    let info = dummy_table_query_info();

    let span = build_table_query_span(
        &info,
        empty_stats(),
        SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        Duration::from_micros(500),
        None,
        None,
        None,
        None,
    );

    // Span shape
    assert_eq!(span.name, "cloud_scan_table");
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

    // Spot-check key values from the dummy info.
    assert_eq!(find_string(&span, "table_id"), Some("tbl-42"));
    assert_eq!(find_string(&span, "table_kind"), Some("lance"));
    assert_eq!(find_string(&span, "caller"), Some("browser_detail_view"));
    assert_eq!(find_int(&span, "schema_total_columns"), Some(12));
    assert_eq!(find_int(&span, "projected_columns"), Some(5));
    assert_eq!(find_bool(&span, "has_limit"), Some(false));
    assert_eq!(find_bool(&span, "is_success"), Some(true));
    assert_eq!(find_int(&span, "total_duration_us"), Some(500));
}

#[test]
fn build_table_query_span_records_scan_stats() {
    let info = dummy_table_query_info();
    let stats = TableScanStatsSnapshot {
        grpc_requests: 7,
        batches: 7,
        rows_returned: 12_345,
        bytes_returned: 4_567_890,
    };

    let span = build_table_query_span(
        &info,
        stats,
        SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        Duration::from_millis(2),
        None,
        None,
        None,
        None,
    );

    assert_eq!(find_int(&span, "fetch_grpc_requests"), Some(7));
    assert_eq!(find_int(&span, "num_record_batches"), Some(7));
    assert_eq!(find_int(&span, "rows_returned"), Some(12_345));
    assert_eq!(find_int(&span, "bytes_returned"), Some(4_567_890));
}

#[test]
fn build_table_query_span_emits_optional_attributes_when_present() {
    let trace_id = opentelemetry::TraceId::from_bytes([3u8; 16]);
    let mut info = dummy_table_query_info();
    info.has_limit = true;
    info.limit_value = Some(500);

    let span = build_table_query_span(
        &info,
        empty_stats(),
        SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        Duration::from_millis(1),
        Some(Duration::from_micros(50)),
        Some(Duration::from_micros(75)),
        Some(trace_id),
        Some(QueryErrorKind::Decode.as_str()),
    );

    // All optional keys are present.
    let optional = [
        "limit_value",
        "time_to_first_response_us",
        "time_to_first_batch_us",
        "error_kind",
    ];
    let keys = attribute_keys(&span);
    for k in optional {
        assert!(keys.contains(k), "missing optional attribute: {k}");
    }

    // is_success flips to false when error_kind is set.
    assert_eq!(find_bool(&span, "is_success"), Some(false));

    assert_eq!(find_int(&span, "limit_value"), Some(500));
    assert_eq!(find_int(&span, "time_to_first_response_us"), Some(50));
    assert_eq!(find_int(&span, "time_to_first_batch_us"), Some(75));
    assert_eq!(find_string(&span, "error_kind"), Some("decode"));
    assert!(span.links.is_empty());
}

#[test]
fn build_table_query_span_uses_wall_clock_range() {
    let info = dummy_table_query_info();
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(2);
    let end = SystemTime::UNIX_EPOCH + Duration::from_millis(2_500);

    let span = build_table_query_span(
        &info,
        empty_stats(),
        start..end,
        Duration::from_micros(0),
        None,
        None,
        None,
        None,
    );

    assert_eq!(span.start_time_unix_nano, 2_000_000_000);
    assert_eq!(span.end_time_unix_nano, 2_500_000_000);
}

#[test]
fn build_table_query_span_records_table_kind_and_caller_strings() {
    // Walk every variant — protects the bounded-enum string mapping from
    // accidental changes that would silently rename PostHog dimensions.
    let cases = [
        (TableKind::Lance, "lance"),
        (TableKind::SystemEntries, "system_entries"),
        (TableKind::SystemNamespaces, "system_namespaces"),
        (TableKind::Unknown, "unknown"),
    ];
    for (kind, expected) in cases {
        let mut info = dummy_table_query_info();
        info.table_kind = kind;
        let span = build_table_query_span(
            &info,
            empty_stats(),
            SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH,
            Duration::ZERO,
            None,
            None,
            None,
            None,
        );
        assert_eq!(find_string(&span, "table_kind"), Some(expected));
    }

    let cases = [
        (TableQueryCaller::CatalogResolver, "catalog_resolver"),
        (TableQueryCaller::EntriesTable, "entries_table"),
        (TableQueryCaller::BrowserDetailView, "browser_detail_view"),
    ];
    for (caller, expected) in cases {
        let mut info = dummy_table_query_info();
        info.caller = caller;
        let span = build_table_query_span(
            &info,
            empty_stats(),
            SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH,
            Duration::ZERO,
            None,
            None,
            None,
            None,
        );
        assert_eq!(find_string(&span, "caller"), Some(expected));
    }
}

#[test]
fn build_table_query_span_no_limit_value_when_no_limit() {
    // has_limit defaults to false, limit_value to None — the optional
    // `limit_value` attribute must NOT be emitted.
    let info = dummy_table_query_info();
    let span = build_table_query_span(
        &info,
        empty_stats(),
        SystemTime::UNIX_EPOCH..SystemTime::UNIX_EPOCH,
        Duration::ZERO,
        None,
        None,
        None,
        None,
    );
    assert!(!attribute_keys(&span).contains("limit_value"));
}

// ---- TableKind::from(&ProviderDetails) ----

#[test]
fn table_kind_from_lance_provider() {
    assert_matches!(TableKind::from(&lance_provider_details()), TableKind::Lance);
}

#[test]
fn table_kind_from_system_entries_provider() {
    let pd = ProviderDetails::SystemTable(SystemTable {
        kind: SystemTableKind::Entries,
    });
    assert_matches!(TableKind::from(&pd), TableKind::SystemEntries);
}

#[test]
fn table_kind_from_system_namespaces_provider() {
    let pd = ProviderDetails::SystemTable(SystemTable {
        kind: SystemTableKind::Namespaces,
    });
    assert_matches!(TableKind::from(&pd), TableKind::SystemNamespaces);
}

#[test]
fn table_kind_from_system_unspecified_falls_back_to_unknown() {
    let pd = ProviderDetails::SystemTable(SystemTable {
        kind: SystemTableKind::Unspecified,
    });
    assert_matches!(TableKind::from(&pd), TableKind::Unknown);
}

// ---- record_* idempotence ----
//
// All `record_*` setters are advertised as "only the first call has effect".
// These tests pin that contract; subsequent calls must not overwrite.

fn make_pending() -> PendingTableQueryAnalytics {
    let origin: Origin = "rerun+http://localhost:51234".parse().unwrap();
    let analytics = ConnectionAnalytics::disabled_for_test(origin);
    analytics.begin_table_query(dummy_table_query_info(), Instant::now())
}

#[tokio::test]
async fn record_first_response_is_once_only() {
    let pending = make_pending();
    pending.record_first_response();
    let first = pending.inner.time_to_first_response.get().copied().unwrap();
    tokio::time::sleep(Duration::from_millis(2)).await;
    pending.record_first_response();
    let second = pending.inner.time_to_first_response.get().copied().unwrap();
    assert_eq!(first, second, "second call must not overwrite");
}

#[tokio::test]
async fn record_first_batch_is_once_only() {
    let pending = make_pending();
    pending.record_first_batch();
    let first = pending.inner.time_to_first_batch.get().copied().unwrap();
    tokio::time::sleep(Duration::from_millis(2)).await;
    pending.record_first_batch();
    let second = pending.inner.time_to_first_batch.get().copied().unwrap();
    assert_eq!(first, second);
}

#[tokio::test]
async fn record_error_is_once_only() {
    let pending = make_pending();
    pending.record_error(QueryErrorKind::GrpcFetch);
    pending.record_error(QueryErrorKind::Decode);
    assert_eq!(
        pending.inner.error_kind.get().copied(),
        Some(QueryErrorKind::GrpcFetch.as_str())
    );
}

#[tokio::test]
async fn record_trace_id_is_once_only() {
    let pending = make_pending();
    let first = opentelemetry::TraceId::from_bytes([1u8; 16]);
    let second = opentelemetry::TraceId::from_bytes([2u8; 16]);
    pending.record_trace_id(first);
    pending.record_trace_id(second);
    assert_eq!(pending.inner.trace_id.get().copied(), Some(first));
}

#[tokio::test]
async fn record_batch_accumulates_across_calls() {
    let pending = make_pending();
    pending.record_batch(100, 1_000);
    pending.record_batch(50, 500);
    pending.record_batch(0, 0); // empty batch — still counts a request/batch
    let stats = pending.inner.stats.snapshot();
    assert_eq!(stats.grpc_requests, 3);
    assert_eq!(stats.batches, 3);
    assert_eq!(stats.rows_returned, 150);
    assert_eq!(stats.bytes_returned, 1_500);
}
