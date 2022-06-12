use crate::{hash::Hash128, path::Index};

// ----------------------------------------------------------------------------

/// Like `Index` but also includes a precomputed hash.
#[derive(Clone, Debug, Eq)]
pub struct IndexKey {
    hash: Hash128,
    index: Index,
}

impl IndexKey {
    #[inline]
    pub fn new(index: Index) -> Self {
        let hash = Hash128::hash(&index);
        Self { index, hash }
    }

    #[inline]
    pub fn index(&self) -> &Index {
        &self.index
    }
}

// ----------------------------------------------------------------------------

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

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Eq)]
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
    pub fn as_slice(&self) -> &[Index] {
        self.components.as_slice()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        self.components.iter()
    }

    pub fn push(&mut self, index: impl Into<IndexKey>) {
        let index = index.into();

        self.components.push(index.index);
        self.hashes[0] = self.hashes[0].rotate_left(5);
        self.hashes[1] = self.hashes[1].rotate_left(5);
        self.hashes[0] ^= index.hash.first64();
        self.hashes[1] ^= index.hash.second64();
    }

    /// Split off the last component.
    pub fn split_last(mut self) -> (IndexPath, IndexKey) {
        let index = IndexKey::new(self.components.pop().unwrap());
        self.hashes[0] ^= index.hash.first64();
        self.hashes[1] ^= index.hash.second64();
        self.hashes[0] = self.hashes[0].rotate_right(5);
        self.hashes[1] = self.hashes[1].rotate_right(5);
        (self, index)
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

    let (key0_again, seq0) = key1.clone().split_last();
    assert_eq!(key0_again.components.len(), 0);
    assert_eq!(key0_again, key0);
    assert_eq!(seq0, IndexKey::new(Index::Sequence(0)));

    let (key1_again, seq1) = key2.split_last();
    assert_eq!(key1_again.components.len(), 1);
    assert_eq!(key1_again, key1);
    assert_eq!(seq1, IndexKey::new(Index::Sequence(1)));
}
