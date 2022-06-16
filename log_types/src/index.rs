use crate::hash::Hash128;

// ----------------------------------------------------------------------------

/// The key of a table.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Index {
    /// For arrays, assumed to be dense (0, 1, 2, …).
    Sequence(u64),

    /// X,Y pixel coordinates, from top left.
    Pixel([u64; 2]),

    /// Any integer, e.g. a hash or an arbitrary identifier.
    Integer(i128),

    /// UUID/GUID
    Uuid(uuid::Uuid),

    /// Anything goes.
    String(String),

    /// Used as the last index when logging a batch of data.
    Placeholder,
}

impl Index {
    #[inline]
    pub fn is_placeholder(&self) -> bool {
        matches!(self, Self::Placeholder)
    }

    #[inline]
    pub fn hash(&self) -> IndexHash {
        IndexHash::hash(self)
    }
}

impl std::fmt::Display for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequence(seq) => format!("#{seq}").fmt(f),
            Self::Pixel([x, y]) => format!("[{x}, {y}]").fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::Uuid(value) => value.fmt(f),
            Self::String(value) => format!("{value:?}").fmt(f), // put it in quotes
            Self::Placeholder => '_'.fmt(f),                    // put it in quotes
        }
    }
}

crate::impl_into_enum!(String, Index, String);

// ----------------------------------------------------------------------------

/// A 128 bit hash of [`Index`] and [`IndexKey`] with negligible chance of collision.
#[derive(Copy, Clone, Eq)]
pub struct IndexHash(Hash128);

impl IndexHash {
    #[inline]
    pub fn hash(index: &Index) -> Self {
        if index.is_placeholder() {
            Self(Hash128::ZERO)
        } else {
            Self(Hash128::hash(index))
        }
    }

    /// Is this equal to `IndexHash::hash(&Index::Placeholder)` ?
    #[inline]
    pub fn is_placeholder(&self) -> bool {
        self.0 == Hash128::ZERO
    }

    #[inline]
    pub fn hash64(&self) -> u64 {
        self.0.hash64()
    }

    #[inline]
    pub fn first64(&self) -> u64 {
        self.0.first64()
    }

    #[inline]
    pub fn second64(&self) -> u64 {
        self.0.second64()
    }
}

impl std::hash::Hash for IndexHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.hash64());
    }
}

impl std::cmp::PartialEq for IndexHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl nohash_hasher::IsEnabled for IndexHash {}

impl std::fmt::Debug for IndexHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "IndexHash({:016X}{:016X})",
            self.0.first64(),
            self.0.second64()
        ))
    }
}

// ----------------------------------------------------------------------------

/// Like [`Index`] but also includes a precomputed hash.
#[derive(Clone, Eq)]
pub struct IndexKey {
    hash: IndexHash,
    index: Index,
}

impl IndexKey {
    #[inline]
    pub fn new(index: Index) -> Self {
        Self {
            hash: IndexHash::hash(&index),
            index,
        }
    }

    #[inline]
    pub fn index(&self) -> &Index {
        &self.index
    }

    #[inline]
    pub fn hash(&self) -> &IndexHash {
        &self.hash
    }

    #[inline]
    pub fn hash64(&self) -> u64 {
        self.hash.hash64()
    }

    #[inline]
    pub(crate) fn into_hash_and_index(self) -> (IndexHash, Index) {
        (self.hash, self.index)
    }

    #[inline]
    pub fn is_placeholder(&self) -> bool {
        self.index.is_placeholder()
    }
}

impl std::cmp::PartialOrd for IndexKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.index.partial_cmp(&other.index)
    }
}

impl std::cmp::Ord for IndexKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index.cmp(&other.index)
    }
}

impl std::cmp::PartialEq for IndexKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash // much faster, and low chance of collision
    }
}

impl std::hash::Hash for IndexKey {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl nohash_hasher::IsEnabled for IndexKey {}

impl From<Index> for IndexKey {
    #[inline]
    fn from(index: Index) -> Self {
        IndexKey::new(index)
    }
}

impl std::fmt::Debug for IndexKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.index.fmt(f)
    }
}
