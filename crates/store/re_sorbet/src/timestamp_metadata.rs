//! Tools for adding timpestamp metadata to a Record Batch.
//!
//! This is used for latency measurements.

use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

use crate::ArrowBatchMetadata;

/// Important stops along the data transform part, from SDK to viewer.
///
/// This is used to annotate timestamps for latency measurements.
///
/// Ordered chronologically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, strum::EnumIter)]
pub enum TimestampLocation {
    /// Time of log call. Encoded in [`re_chunk::RowId`].
    Log,

    /// When the batcher has created the chunk.
    ///
    /// Encoded in [`re_chunk::ChunkId`].
    ChunkCreation,

    /// When was this batch sent by the SDK gRPC log sink?
    IPCEncode,

    /// When was this batch last decoded from IPC bytes by the gRPC server (presumably in the viewer)?
    IPCDecode,

    /// When the data was ingested into the store in the viewer.
    Ingest,
}

impl TimestampLocation {
    /// The first step of the pipeline
    pub const FIRST: Self = Self::Log;

    /// The last step of the pipeline
    pub const LAST: Self = Self::Ingest;

    /// Get the arrow recordbartch metadata key associated with this timestamp location.
    ///
    /// Returns `None` for timestamp locations that are not recorded in metadata.
    pub fn metadata_key(&self) -> Option<&'static str> {
        #[expect(clippy::match_same_arms)]
        match self {
            Self::Log => None,           // encoded in RowId
            Self::ChunkCreation => None, // encoded in ChunkId
            Self::IPCEncode => Some("rerun:timestamp_sdk_ipc_encoded"),
            Self::IPCDecode => Some("rerun:timestamp_viewer_ipc_decoded"),
            Self::Ingest => None, // not recorded
        }
    }
}

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
pub struct TimestampMetadata(BTreeMap<TimestampLocation, web_time::SystemTime>);

impl Deref for TimestampMetadata {
    type Target = BTreeMap<TimestampLocation, web_time::SystemTime>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TimestampMetadata {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl TimestampMetadata {
    pub fn parse_record_batch_metadata(metadata: &ArrowBatchMetadata) -> Self {
        use strum::IntoEnumIterator as _;

        let mut map = BTreeMap::new();

        for location in TimestampLocation::iter() {
            if let Some(key) = location.metadata_key()
                && let Some(value) = metadata.get(key)
                && let Some(timestamp) = parse_timestamp(value.as_str())
            {
                map.insert(location, timestamp);
            }
        }

        Self(map)
    }

    pub fn to_metadata(&self) -> impl Iterator<Item = (String, String)> {
        let mut metadata = Vec::new();

        for (location, timestamp) in &self.0 {
            if let Some(key) = location.metadata_key() {
                metadata.push((key.to_owned(), encode_timestamp(*timestamp)));
            }
        }

        metadata.into_iter()
    }
}
