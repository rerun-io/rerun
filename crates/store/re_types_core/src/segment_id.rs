/// Identifies a single segment within a dataset.
///
/// Wraps a string id so the type system distinguishes segment identifiers from
/// arbitrary strings.
///
/// Each segment is an episode, potentially consisting of many layers,
/// each backed by its own .rrd file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SegmentId {
    pub id: String,
}

impl SegmentId {
    #[inline]
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

impl From<String> for SegmentId {
    fn from(id: String) -> Self {
        Self { id }
    }
}

impl From<&str> for SegmentId {
    fn from(id: &str) -> Self {
        Self { id: id.to_owned() }
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
        self.id.as_str()
    }
}
