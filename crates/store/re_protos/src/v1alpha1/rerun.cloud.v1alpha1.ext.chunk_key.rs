//! Extension types and helpers for `ChunkKey`, `RrdChunkLocation`, and the
//! related ETag / URL utilities used on both the server (`FetchChunks`) and
//! the OSS direct-fetch client paths.
//!
//! Split out of the main `rerun.cloud.v1alpha1.ext.rs` to keep that file
//! under the size cap enforced by `scripts/ci/check_large_files.py`.

use crate::cloud::v1alpha1::ext::DataSourceKind;
use crate::{TypeConversionError, invalid_field, missing_field};

// --- ChunkKey / RrdChunkLocation ---

/// Decoded form of [`crate::cloud::v1alpha1::ChunkKey`].
///
/// The `location` payload is opaque on the wire and is interpreted per
/// [`crate::cloud::v1alpha1::DataSourceKind`] (e.g. as [`RrdChunkLocation`]
/// for RRD-backed partitions).
#[derive(Debug, Clone)]
pub struct ChunkKey {
    pub chunk_id: re_chunk::ChunkId,
    pub data_source_kind: DataSourceKind,
    pub location: Vec<u8>,

    /// `ETag` of the source object as observed at registration time, when available.
    ///
    /// Legacy registrations and stores that do not return an `ETag` leave this `None`.
    pub etag: Option<ETag>,

    /// Wall-clock registration time of the parent segment, as recorded in
    /// the dataset manifest.
    ///
    /// Diagnostic only.
    pub registration_time: Option<jiff::Timestamp>,
}

impl ChunkKey {
    pub fn as_bytes(&self) -> Vec<u8> {
        use prost::Message as _;

        let chunk_key: crate::cloud::v1alpha1::ChunkKey = self.clone().into();
        chunk_key.encode_to_vec()
    }
}

impl TryFrom<crate::cloud::v1alpha1::ChunkKey> for ChunkKey {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::ChunkKey) -> Result<Self, Self::Error> {
        let tuid = value
            .chunk_id
            .ok_or(missing_field!(crate::cloud::v1alpha1::ChunkKey, "chunk_id"))?;
        let id: re_tuid::Tuid = tuid.try_into()?;
        let chunk_id = re_chunk::ChunkId::from_u128(id.as_u128());

        let data_source_kind = DataSourceKind::try_from(value.data_source_kind)?;

        let location = value
            .location
            .ok_or(missing_field!(crate::cloud::v1alpha1::ChunkKey, "location"))?
            .as_ref()
            .to_vec();

        Ok(Self {
            chunk_id,
            data_source_kind,
            location,
            etag: value.etag.map(ETag::new),
            registration_time: value
                .registration_time_nanos
                .and_then(|n| jiff::Timestamp::from_nanosecond(n as i128).ok()),
        })
    }
}

impl TryFrom<&[u8]> for ChunkKey {
    type Error = TypeConversionError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        use prost::Message as _;

        let proto_chunk_key = crate::cloud::v1alpha1::ChunkKey::decode(bytes)
            .map_err(TypeConversionError::DecodeError)?;

        proto_chunk_key.try_into()
    }
}

impl From<ChunkKey> for crate::cloud::v1alpha1::ChunkKey {
    fn from(value: ChunkKey) -> Self {
        let tuid =
            crate::common::v1alpha1::Tuid::from(re_tuid::Tuid::from_u128(value.chunk_id.as_u128()));
        let location = prost::bytes::Bytes::from_owner(value.location);
        let data_source_kind: crate::cloud::v1alpha1::DataSourceKind =
            value.data_source_kind.into();

        Self {
            chunk_id: Some(tuid),
            data_source_kind: data_source_kind as i32,
            location: Some(location),
            etag: value.etag.map(Into::into),
            registration_time_nanos: value
                .registration_time
                .and_then(|t| i64::try_from(t.as_nanosecond()).ok()),
        }
    }
}

/// Decoded form of [`crate::cloud::v1alpha1::RrdChunkLocation`].
#[derive(Debug, Clone)]
pub struct RrdChunkLocation {
    pub url: url::Url,
    pub offset: u64,
    pub length: u64,
}

impl RrdChunkLocation {
    pub fn as_bytes(&self) -> Vec<u8> {
        use prost::Message as _;

        let rrd_location: crate::cloud::v1alpha1::RrdChunkLocation = self.clone().into();
        rrd_location.encode_to_vec()
    }
}

impl TryFrom<crate::cloud::v1alpha1::RrdChunkLocation> for RrdChunkLocation {
    type Error = TypeConversionError;

    fn try_from(value: crate::cloud::v1alpha1::RrdChunkLocation) -> Result<Self, Self::Error> {
        let url = value
            .url
            .ok_or(missing_field!(
                crate::cloud::v1alpha1::RrdChunkLocation,
                "url"
            ))?
            .parse()
            .map_err(|err: url::ParseError| {
                invalid_field!(
                    crate::cloud::v1alpha1::RrdChunkLocation,
                    "url",
                    err.to_string()
                )
            })?;

        let offset = value.offset.ok_or(missing_field!(
            crate::cloud::v1alpha1::RrdChunkLocation,
            "offset"
        ))?;

        let length = value.length.ok_or(missing_field!(
            crate::cloud::v1alpha1::RrdChunkLocation,
            "length"
        ))?;

        Ok(Self {
            url,
            offset,
            length,
        })
    }
}

impl From<RrdChunkLocation> for crate::cloud::v1alpha1::RrdChunkLocation {
    fn from(value: RrdChunkLocation) -> Self {
        Self {
            url: Some(value.url.to_string()),
            offset: Some(value.offset),
            length: Some(value.length),
        }
    }
}

impl TryFrom<&[u8]> for RrdChunkLocation {
    type Error = TypeConversionError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        use prost::Message as _;

        let proto_location = crate::cloud::v1alpha1::RrdChunkLocation::decode(bytes)
            .map_err(TypeConversionError::DecodeError)?;

        proto_location.try_into()
    }
}

// --- ETag ---

/// User-facing message returned (server-side) and surfaced (client-side) when
/// drift between the registered source object and the live one is detected.
pub const SOURCE_CHANGED_MESSAGE: &str = "the source object has changed since this dataset was registered; re-register to pick up the new version";

/// Typed wrapper around an HTTP `ETag` value (RFC 7232).
///
/// Preserves the optional `W/` prefix verbatim because it carries a real
/// signal: for opaque blob storage, real backends emit strong `ETags`, and a
/// `W/` prefix on the response usually means an intermediary (CDN, edge
/// cache, transparent compressor) re-encoded or transformed the bytes — in
/// which case the bytes we'll decode may not match what the manifest indexed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ETag(String);

impl ETag {
    /// Wrap a raw `ETag` value (with or without `W/` prefix). Surrounding
    /// whitespace is trimmed.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into().trim().to_owned())
    }

    /// Raw value as it would appear on the wire (`"abc"` or `W/"abc"`).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `true` if this is a strong `ETag` (no `W/` prefix).
    ///
    /// RFC 7232 §3.1 mandates strong comparison for `If-Match`; sending a
    /// weak `ETag` in `If-Match` is spec-invalid and some servers reject
    /// with 400 or 412.
    pub fn is_strong(&self) -> bool {
        !self.0.starts_with("W/")
    }

    /// Returns the raw value if this `ETag` is strong, or `None` if weak.
    /// Use to gate sending of `If-Match` headers in one shot.
    pub fn as_if_match(&self) -> Option<&str> {
        self.is_strong().then_some(self.as_str())
    }

    /// Compare this `ETag` (the manifest-registered, "expected" value)
    /// against `actual` (the live response value) using **symmetric strict
    /// comparison**: any prefix transition (`W/` ↔ no `W/`) signals that
    /// the server changed its representation claim, which we treat as drift.
    /// Identical-prefix + same value matches.
    pub fn matches(&self, actual: &Self) -> bool {
        let expected = self.0.as_str();
        let actual = actual.0.as_str();
        let exp_weak = expected.starts_with("W/");
        let act_weak = actual.starts_with("W/");
        if exp_weak != act_weak {
            return false;
        }
        fn strip(s: &str) -> &str {
            let s = s.strip_prefix("W/").unwrap_or(s);
            s.trim_start_matches('"').trim_end_matches('"')
        }
        strip(expected) == strip(actual)
    }
}

impl std::fmt::Display for ETag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for ETag {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for ETag {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<ETag> for String {
    fn from(e: ETag) -> Self {
        e.0
    }
}

// --- URL log redaction ---

/// Strip the query string (and everything after `?`) from a URL.
pub fn url_strip_query(url: &str) -> &str {
    url.split_once('?').map_or(url, |(prefix, _)| prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn et(s: &str) -> ETag {
        ETag::new(s)
    }

    #[test]
    fn etag_matches_strong_identical() {
        assert!(et("\"abc\"").matches(&et("\"abc\"")));
    }

    #[test]
    fn etag_matches_weak_identical() {
        // RR-4549 regression: identical weak ETags must match. Earlier
        // implementation rejected any input starting with `W/`.
        assert!(et("W/\"abc\"").matches(&et("W/\"abc\"")));
    }

    #[test]
    fn etag_matches_strong_to_weak_downgrade_is_drift() {
        // Server flipped strong → weak. Likely cause: a CDN or proxy
        // transformed the response (compression, partial content, etc.).
        assert!(!et("\"abc\"").matches(&et("W/\"abc\"")));
    }

    #[test]
    fn etag_matches_weak_to_strong_upgrade_is_drift() {
        // Server upgraded weak → strong: representation claim changed.
        // We can't retroactively know whether the registered weak bytes
        // match the now-strong bytes — treat as drift.
        assert!(!et("W/\"abc\"").matches(&et("\"abc\"")));
    }

    #[test]
    fn etag_matches_different_values() {
        assert!(!et("\"abc\"").matches(&et("\"def\"")));
        assert!(!et("W/\"abc\"").matches(&et("W/\"def\"")));
        assert!(!et("\"abc\"").matches(&et("W/\"def\"")));
        assert!(!et("W/\"abc\"").matches(&et("\"def\"")));
    }

    #[test]
    fn etag_matches_whitespace_tolerant() {
        // Construction trims; matches works on the stored value.
        assert!(et("  \"abc\"  ").matches(&et("\"abc\"")));
        assert!(et("\"abc\"").matches(&et("\t\"abc\"\n")));
    }

    #[test]
    fn etag_is_strong_basics() {
        assert!(et("\"abc\"").is_strong());
        assert!(!et("W/\"abc\"").is_strong());
        // Whitespace doesn't fool the strong check (trimmed at construction).
        assert!(!et("  W/\"abc\"").is_strong());
    }

    #[test]
    fn etag_as_if_match_gates_on_strong() {
        assert_eq!(et("\"abc\"").as_if_match(), Some("\"abc\""));
        assert_eq!(et("W/\"abc\"").as_if_match(), None);
    }

    #[test]
    fn url_strip_query_basics() {
        assert_eq!(
            url_strip_query("https://bucket.s3/key?x=1&y=2"),
            "https://bucket.s3/key"
        );
        assert_eq!(
            url_strip_query("https://bucket.s3/key"),
            "https://bucket.s3/key"
        );
        assert_eq!(url_strip_query(""), "");
    }
}
