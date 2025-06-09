//! Tools for adding timing metadata to a Record Batch.
//!
//! This is used for latency measurements.

/// When was this batch last encoded into IPC bytes?
/// Usually this is when the batch was last sent over gRPC.
pub const LAST_ENCODED_AT: &str = "sorbet.encoded_at";

/// When was this batch last decoded from IPC bytes?
/// Usually this is when the batch was last received over gRPC.
pub const LAST_DECODED_AT: &str = "sorbet.decoded_at";

/// We encode time as seconds since the Unix epoch,
/// with nanosecond precision, e.g. `1700000000.012345678`.
pub fn now_timestamp() -> String {
    encode_timestamp(web_time::SystemTime::now())
}

/// We encode time as seconds since the Unix epoch,
/// with nanosecond precision, e.g. `1700000000.012345678`.
pub fn encode_timestamp(timestamp: web_time::SystemTime) -> String {
    let duration = timestamp
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos())
}

/// We encode time as seconds since the Unix epoch,
/// with nanosecond precision, e.g. `1700000000.012345678`.
pub fn parse_timestamp(timestamp: &str) -> Option<web_time::SystemTime> {
    let parts: Vec<&str> = timestamp.split('.').collect();
    if parts.len() != 2 {
        return None;
    }

    let seconds = parts[0].parse::<u64>().ok()?;
    let nanos = parts[1].parse::<u32>().ok()?;

    Some(web_time::UNIX_EPOCH + web_time::Duration::new(seconds, nanos))
}

#[test]
fn test_timestamp_encoding() {
    let now = web_time::SystemTime::now();
    let encoded = encode_timestamp(now);
    assert_eq!(encoded.len(), 20); // e.g. "1700000000.012345678"
    let parsed = parse_timestamp(&encoded).unwrap();
    assert_eq!(parsed, now);
}
