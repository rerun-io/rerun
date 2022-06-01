mod log_store;
mod scene;
mod storage;

pub use log_store::*;
pub use scene::*;
pub use storage::*;

pub use log_types::{DataPath, DataPathComponent, Index, TypePath, TypePathComponent};

pub enum AtomType {
    // 1D:
    I32,
    F32,

    Color,

    // ----------------------------
    // 2D:
    Pos2,
    BBox2D,
    LineSegment2D,
    Image,

    // ----------------------------
    // 3D:
    Pos3,
    BBox3,
    Path3D,
    LineSegment3D,
    Mesh3D,
    Camera,

    // ----------------------------
    // N-D:
    Vecf32,
}

#[derive(Clone, Copy, Debug)]
pub enum Data {
    F32(f32),
    Pos3([f32; 3]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StructType {
    /// ```ignore
    /// pos: Pos3,
    /// radius: Option<F32>,
    /// color: Option<Color>,
    /// ```
    Point3D,
}

pub fn into_type_path(data_path: DataPath) -> (TypePath, IndexPathKey) {
    let mut type_path = im::Vector::default();
    let mut index_path = IndexPathKey::default();
    for component in data_path {
        match component {
            DataPathComponent::String(name) => {
                type_path.push_back(TypePathComponent::String(name));
            }
            DataPathComponent::Index(index) => {
                type_path.push_back(TypePathComponent::Index);
                index_path.push_back(index);
            }
        }
    }
    (type_path, index_path)
}

#[allow(dead_code)]
pub(crate) fn data_path_from_type_and_index(
    type_path: &TypePath,
    index_path: &IndexPathKey,
) -> DataPath {
    let mut index_it = index_path.components.iter();

    let ret = DataPath(
        type_path
            .iter()
            .map(|typ| match typ {
                TypePathComponent::String(name) => DataPathComponent::String(*name),
                TypePathComponent::Index => {
                    DataPathComponent::Index(index_it.next().expect("Bad type/index split").clone())
                }
            })
            .collect(),
    );

    assert!(index_it.next().is_none(), "Bad type/index split");

    ret
}

#[allow(dead_code)]
pub(crate) fn data_path_from_type_and_index_prefix(
    type_path: &TypePath,
    index_prefix: &IndexPathKey,
    last_index: &Index,
) -> DataPath {
    let mut index_it = index_prefix.components.iter();

    let ret = DataPath(
        type_path
            .iter()
            .map(|typ| match typ {
                TypePathComponent::String(name) => DataPathComponent::String(*name),
                TypePathComponent::Index => DataPathComponent::Index(
                    index_it
                        .next()
                        .cloned()
                        .unwrap_or_else(|| last_index.clone()),
                ),
            })
            .collect(),
    );

    assert!(index_it.next().is_none(), "Bad type/index split");

    ret
}

/// Path to the object owning the batch, i.e. stopping before the last index
pub(crate) fn batch_parent_object_path(
    type_path: &TypePath,
    index_path_prefix: &IndexPathKey,
) -> DataPath {
    let mut index_it = index_path_prefix.components.iter();

    let mut components = vec![];

    for typ in type_path {
        match typ {
            TypePathComponent::String(name) => {
                components.push(DataPathComponent::String(*name));
            }
            TypePathComponent::Index => {
                if let Some(index) = index_it.next() {
                    components.push(DataPathComponent::Index(index.clone()));
                } else {
                    return DataPath(components);
                }
            }
        }
    }

    panic!("Not a batch path");
}
// ----------------------------------------------------------------------------

/// Like `Index` but also includes a precomputed hash.
#[derive(Clone, Debug, Eq, PartialOrd, Ord)]
pub struct IndexKey {
    index: Index,
    hashes: [u64; 2], // 128 bit to avoid collisions
}

impl IndexKey {
    #[inline]
    pub fn new(index: Index) -> Self {
        let hashes = double_hash(&index);
        Self { index, hashes }
    }

    pub fn index(&self) -> &Index {
        &self.index
    }
}

impl std::cmp::PartialEq for IndexKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hashes == other.hashes // much faster, and low chance of collision
    }
}

impl std::hash::Hash for IndexKey {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hashes[0]);
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

#[derive(Clone, Debug, Default, Eq, PartialOrd, Ord)]
pub struct IndexPathKey {
    components: im::Vector<Index>,
    hashes: [u64; 2], // 128 bit to avoid collisions
}

impl IndexPathKey {
    #[inline]
    pub fn new(components: im::Vector<Index>) -> Self {
        let mut slf = Self::default();
        for index in components {
            slf.push_back(index);
        }
        slf
    }

    pub fn push_back(&mut self, index: impl Into<IndexKey>) {
        let index = index.into();

        self.components.push_back(index.index);
        self.hashes[0] = self.hashes[0].rotate_left(5);
        self.hashes[1] = self.hashes[1].rotate_left(5);
        self.hashes[0] ^= index.hashes[0];
        self.hashes[1] ^= index.hashes[1];
    }

    /// Split off the last component.
    pub fn split_last(mut self) -> (IndexPathKey, IndexKey) {
        let index = IndexKey::new(self.components.pop_back().unwrap());
        self.hashes[0] ^= index.hashes[0];
        self.hashes[1] ^= index.hashes[1];
        self.hashes[0] = self.hashes[0].rotate_right(5);
        self.hashes[1] = self.hashes[1].rotate_right(5);
        (self, index)
    }
}

impl std::cmp::PartialEq for IndexPathKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hashes == other.hashes // much faster, and low chance of collision
    }
}

impl std::hash::Hash for IndexPathKey {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hashes[0]);
    }
}

impl nohash_hasher::IsEnabled for IndexPathKey {}

#[test]
fn test_index_path_key() {
    let key0 = IndexPathKey::default();

    let mut key1 = key0.clone();
    key1.push_back(Index::Sequence(0));
    let key1 = key1;

    let mut key2 = key1.clone();
    key2.push_back(Index::Sequence(1));
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

// ----------------------------------------------------------------------------

#[inline]
fn double_hash(value: impl std::hash::Hash + Copy) -> [u64; 2] {
    [hash_with_seed(value, 123), hash_with_seed(value, 456)]
}

/// Hash the given value.
#[inline]
fn hash_with_seed(value: impl std::hash::Hash, seed: u128) -> u64 {
    use std::hash::Hasher as _;
    let mut hasher = ahash::AHasher::new_with_keys(666, seed);
    value.hash(&mut hasher);
    hasher.finish()
}

// ----------------------------------------------------------------------------

/// A query in time.
pub enum TimeQuery<Time> {
    /// Get the latest version of the data available at this time.
    LatestAt(Time),

    /// Get all the data within this time interval.
    Range(std::ops::RangeInclusive<Time>),
}

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
        puffin::profile_scope!($($arg)*);
    };
}
