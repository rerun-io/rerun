//! Tools for adding timing metadata to a Record Batch.
//!
//! This is used for latency measurements.

use crate::ArrowBatchMetadata;

/// When was this batch last encoded into IPC bytes?
/// Usually this is when the batch was last sent over gRPC.
pub const KEY_TIMESTAMP_IPC_ENCODED: &str = "sorbet.timestamp_ipc_encoded";

/// When was this batch last decoded from IPC bytes?
/// Usually this is when the batch was last received over gRPC.
pub const KEY_TIMESTAMP_IPC_DECODED: &str = "sorbet.timestamp_ipc_decoded";

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

/// Timestamps about this batch; used for latency measurements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimestampMetadata {
    /// When was this batch last encoded into IPC bytes?
    pub last_encoded_at: Option<web_time::SystemTime>,

    /// When was this batch last decoded from IPC bytes?
    pub last_decoded_at: Option<web_time::SystemTime>,
}

impl TimestampMetadata {
    pub fn parse_record_batch_metadata(metadata: &ArrowBatchMetadata) -> Self {
        let last_encoded_at = metadata
            .get(KEY_TIMESTAMP_IPC_ENCODED)
            .and_then(|s| parse_timestamp(s.as_str()));
        let last_decoded_at = metadata
            .get(KEY_TIMESTAMP_IPC_DECODED)
            .and_then(|s| parse_timestamp(s.as_str()));

        Self {
            last_encoded_at,
            last_decoded_at,
        }
    }

    pub fn to_metadata(&self) -> impl Iterator<Item = (String, String)> {
        let Self {
            last_encoded_at,
            last_decoded_at,
        } = self;

        let mut metadata = Vec::new();

        if let Some(last_encoded_at) = last_encoded_at {
            metadata.push((
                KEY_TIMESTAMP_IPC_ENCODED.to_owned(),
                encode_timestamp(*last_encoded_at),
            ));
        }

        if let Some(last_decoded_at) = last_decoded_at {
            metadata.push((
                KEY_TIMESTAMP_IPC_DECODED.to_owned(),
                encode_timestamp(*last_decoded_at),
            ));
        }

        metadata.into_iter()
    }
}
