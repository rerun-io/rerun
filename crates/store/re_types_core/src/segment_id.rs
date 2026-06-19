use std::borrow::Cow;

/// Identifies a single segment within a dataset.
///
/// Wraps a string id so the type system distinguishes segment identifiers from
/// arbitrary strings.
///
/// Each segment is an episode, potentially consisting of many layers,
/// each backed by its own .rrd file.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    re_byte_size::SizeBytes,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct SegmentId {
    id: String,
}

impl SegmentId {
    #[inline]
    pub fn new(id: String) -> Self {
        Self { id }
    }

    pub fn as_str(&self) -> &str {
        &self.id
    }

    pub fn into_inner(self) -> String {
        self.id
    }
}

impl From<SegmentId> for String {
    fn from(value: SegmentId) -> Self {
        value.id
    }
}

impl From<String> for SegmentId {
    fn from(id: String) -> Self {
        Self { id }
    }
}

// Make `quiver::Column<SegmentId>` work (backed by a `Utf8` column):
quiver::newtype_datatype!(SegmentId, quiver::Utf8);

impl From<&str> for SegmentId {
    fn from(id: &str) -> Self {
        Self { id: id.to_owned() }
    }
}

impl<'a> From<Cow<'a, str>> for SegmentId {
    fn from(id: Cow<'a, str>) -> Self {
        Self {
            id: id.into_owned(),
        }
    }
}

impl std::fmt::Display for SegmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
    }
}

impl AsRef<str> for SegmentId {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Allows `&str` lookups in maps keyed by [`SegmentId`].
///
/// Sound because the derived `Eq`/`Ord`/`Hash` all delegate to the inner `String`,
/// matching `str` semantics — same contract as `String: Borrow<str>`.
impl std::borrow::Borrow<str> for SegmentId {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
