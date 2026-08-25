use std::collections::HashSet;

use super::*;

fn dummy_info_with_filters(offered: u32, signatures: &str) -> TableQueryInfo {
    TableQueryInfo {
        table_id: "tbl-1".to_owned(),
        table_kind: TableKind::Lance,
        caller: TableQueryCaller::CatalogResolver,
        schema_total_columns: 4,
        projected_columns: 4,
        has_limit: false,
        limit_value: None,
        time_range: web_time::SystemTime::UNIX_EPOCH
            ..web_time::SystemTime::UNIX_EPOCH + web_time::Duration::from_secs(1),
        filters_total: offered,
        filters_signatures: signatures.to_owned(),
    }
}

fn attribute_keys(span: &opentelemetry_proto::tonic::trace::v1::Span) -> HashSet<&str> {
    span.attributes.iter().map(|kv| kv.key.as_str()).collect()
}

fn find_int(span: &opentelemetry_proto::tonic::trace::v1::Span, key: &str) -> Option<i64> {
    use opentelemetry_proto::tonic::common::v1::any_value::Value;
    span.attributes
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
            Value::IntValue(i) => Some(*i),
            _ => None,
        })
}

fn find_string<'a>(
    span: &'a opentelemetry_proto::tonic::trace::v1::Span,
    key: &str,
) -> Option<&'a str> {
    use opentelemetry_proto::tonic::common::v1::any_value::Value;
    span.attributes
        .iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| match kv.value.as_ref()?.value.as_ref()? {
            Value::StringValue(s) => Some(s.as_str()),
            _ => None,
        })
}

#[test]
fn filters_omitted_when_none_offered() {
    let info = dummy_info_with_filters(0, "");
    let span = build_table_query_span(
        &info,
        TableScanStatsSnapshot::default(),
        web_time::SystemTime::UNIX_EPOCH
            ..web_time::SystemTime::UNIX_EPOCH + web_time::Duration::from_secs(1),
        web_time::Duration::ZERO,
        None,
        None,
        None,
        None,
    );
    let keys = attribute_keys(&span);
    assert!(!keys.contains("filters_total"), "must be absent when zero");
    assert!(
        !keys.contains("filters_signatures"),
        "must be absent when empty"
    );
}

#[test]
fn filters_emitted_when_present() {
    // Signatures reach this struct already scrubbed by `expr_filter_signature`.
    let sigs = "(frame_nr > ?);rerun_segment_id IN (?, ?)";
    let info = dummy_info_with_filters(2, sigs);
    let span = build_table_query_span(
        &info,
        TableScanStatsSnapshot::default(),
        web_time::SystemTime::UNIX_EPOCH
            ..web_time::SystemTime::UNIX_EPOCH + web_time::Duration::from_secs(1),
        web_time::Duration::ZERO,
        None,
        None,
        None,
        None,
    );
    assert_eq!(find_int(&span, "filters_total"), Some(2));
    assert_eq!(find_string(&span, "filters_signatures"), Some(sigs));
}
