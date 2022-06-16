use crate::{Index, IndexHash, IndexKey};

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

    pub fn replace_last_placeholder_with(&mut self, index_hash: &IndexHash) {
        // `Index::Placeholder` has zero as hash, so we can easily replace it:
        assert!(!index_hash.is_placeholder());
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

#[derive(Clone, Default, Eq)]
pub struct IndexPath {
    components: Vec<Index>,
    hashes: [u64; 2], // 128 bit to avoid collisions
}

impl IndexPath {
    #[inline]
    pub fn new(components: Vec<Index>) -> Self {
        let mut slf = Self::default();
        for index in components {
            slf.push(index);
        }
        slf
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

    pub fn push(&mut self, index: impl Into<IndexKey>) {
        let index = index.into();
        let (hash, index) = index.into_hash_and_index();

        self.components.push(index);
        self.hashes[0] = self.hashes[0].rotate_left(5);
        self.hashes[1] = self.hashes[1].rotate_left(5);
        self.hashes[0] ^= hash.first64();
        self.hashes[1] ^= hash.second64();
    }

    pub fn pop(&mut self) -> Option<IndexKey> {
        let index = IndexKey::new(self.components.pop()?);
        self.hashes[0] ^= index.hash().first64();
        self.hashes[1] ^= index.hash().second64();
        self.hashes[0] = self.hashes[0].rotate_right(5);
        self.hashes[1] = self.hashes[1].rotate_right(5);
        Some(index)
    }

    /// Replace last component with [`Index::Placeholder`], and return what was there.
    pub fn replace_last_with_placeholder(mut self) -> (IndexPath, IndexKey) {
        let index = self.pop().unwrap();
        assert_ne!(index, IndexKey::new(Index::Placeholder));
        self.push(Index::Placeholder);
        (self, index)
    }

    /// Replace last [`Index::Placeholder`] with the given key.
    pub fn replace_last_placeholder_with(&mut self, key: IndexKey) {
        let index = self.pop().unwrap();
        assert_eq!(index, IndexKey::new(Index::Placeholder));
        self.push(key);
    }

    /// If true, then this is an index prefix path for use with batches
    pub fn has_placeholder_last(&self) -> bool {
        matches!(self.components.last(), Some(Index::Placeholder))
    }
}

pub type Iter<'a> = std::slice::Iter<'a, Index>;

#[cfg(feature = "serde")]
impl serde::Serialize for IndexPath {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_slice().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for IndexPath {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <Vec<Index>>::deserialize(deserializer).map(IndexPath::new)
    }
}

impl std::cmp::PartialEq for IndexPath {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hashes == other.hashes // much faster, and low chance of collision
    }
}

impl std::hash::Hash for IndexPath {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hashes[0]);
    }
}

impl nohash_hasher::IsEnabled for IndexPath {}

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

    let (key1_prefix, seq0) = key1.replace_last_with_placeholder();
    assert_eq!(key1_prefix.components.len(), 1);
    assert_eq!(seq0, IndexKey::new(Index::Sequence(0)));

    let (key2_prefix, seq1) = key2.replace_last_with_placeholder();
    assert_eq!(key2_prefix.components.len(), 2);
    assert_eq!(seq1, IndexKey::new(Index::Sequence(1)));
}
