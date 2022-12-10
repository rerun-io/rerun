use crate::{Index, IndexHash};

// ----------------------------------------------------------------------------

/// A 128 bit hash of a [`IndexPath`] with negligible risk of collision.
#[derive(Clone, Copy, Default, Eq)]
pub struct IndexPathHash([u64; 2]);

impl IndexPathHash {
    pub fn from_path(path: &IndexPath) -> Self {
        let mut hash = Self::default();
        for index in &path.components {
            hash.push(&index.hash());
        }
        hash
    }

    pub fn push(&mut self, index_hash: &IndexHash) {
        self.0[0] = self.0[0].rotate_left(5);
        self.0[1] = self.0[1].rotate_left(5);
        self.0[0] ^= index_hash.first64();
        self.0[1] ^= index_hash.second64();
    }
}

impl std::hash::Hash for IndexPathHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0[0]);
    }
}

impl PartialEq for IndexPathHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl nohash_hasher::IsEnabled for IndexPathHash {}

impl std::fmt::Debug for IndexPathHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("Hash128({:016X}{:016X})", self.0[0], self.0[1]))
    }
}

// ----------------------------------------------------------------------------

/// The [`Index`]es in an [`crate::ObjPath`].
///
/// An [`crate::ObjPath`] like `camera/"left"/points/#42` can be split up into:
/// * [`TypePath`]: `camera/*/points/*`
/// * [`IndexPath`]: `*/"left"/*/#42`
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct IndexPath {
    components: Vec<Index>,
}

impl IndexPath {
    #[inline]
    pub fn new(components: Vec<Index>) -> Self {
        Self { components }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.components.len()
    }

    #[inline]
    pub fn as_slice(&self) -> &[Index] {
        self.components.as_slice()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        self.components.iter()
    }

    pub fn push(&mut self, index: Index) {
        self.components.push(index);
    }

    pub fn pop(&mut self) -> Option<Index> {
        self.components.pop()
    }
}

pub type Iter<'a> = std::slice::Iter<'a, Index>;

impl std::fmt::Debug for IndexPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(f)
    }
}

#[test]
fn test_index_path_key() {
    let key0 = IndexPath::default();

    let mut key1 = key0.clone();
    key1.push(Index::Sequence(0));
    let key1 = key1;

    let mut key2 = key1.clone();
    key2.push(Index::Sequence(1));
    let key2 = key2;

    assert_eq!(key0.components.len(), 0);
    assert_eq!(key1.components.len(), 1);
    assert_eq!(key2.components.len(), 2);
}
