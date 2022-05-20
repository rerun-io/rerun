mod scene;
mod storage;

pub use scene::*;
pub use storage::*;

use std::collections::BTreeMap;

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

// pub struct TypePath(Vec<TypePathComponent>);
pub type TypePath = im::Vector<TypePathComponent>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypePathComponent {
    /// Struct member
    Name(String),

    /// Table (array/map) member.
    /// Tables are homogenous, so it is the same type path for all.
    Index,
}

// pub struct DataPath(Vec<DataPathComponent>);
pub type DataPath = im::Vector<DataPathComponent>;

pub fn into_type_path(data_path: DataPath) -> (TypePath, IndexPathKey) {
    let mut type_path = im::Vector::default();
    let mut index_path = IndexPathKey::default();
    for component in data_path {
        match component {
            DataPathComponent::Name(name) => {
                type_path.push_back(TypePathComponent::Name(name));
            }
            DataPathComponent::Index(index) => {
                type_path.push_back(TypePathComponent::Index);
                index_path.push_back(index);
            }
        }
    }
    (type_path, index_path)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DataPathComponent {
    /// struct member
    Name(String),

    /// array/table/map member
    Index(Index),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Index {
    /// For arrays, assumed to be dense (0, 1, 2, …)
    Sequence(u64),

    /// X,Y pixel coordinates, from top left.
    Pixel([u64; 2]),

    /// Any integer, e.g. a hash
    Integer(i128),

    /// UUID/GUID
    // Uuid(Uuid),

    /// Anything goes
    String(String),
}

// ----------------------------------------------------------------------------

/// Like `Index` but also includes a precomputed hash.
#[derive(Clone, Debug, Eq, PartialOrd, Ord)]
pub struct IndexKey {
    index: Index,
    hash: u64,
}

impl IndexKey {
    #[inline]
    pub fn new(index: Index) -> Self {
        let hash = hash(&index);
        Self { index, hash }
    }

    pub fn index(&self) -> &Index {
        &self.index
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
        state.write_u64(self.hash);
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
    hash: u64,
}

impl IndexPathKey {
    #[inline]
    pub fn new(components: im::Vector<Index>) -> Self {
        let hash = hash(&components);
        Self { components, hash }
    }

    pub fn push_back(&mut self, comp: Index) {
        self.components.push_back(comp);
        self.hash = hash(&self.components);
    }

    /// Split off the last component.
    pub fn split_last(&self) -> (IndexPathKey, Index) {
        let mut head = self.components.clone();
        let tail = head.pop_back().unwrap();
        (IndexPathKey::new(head), tail) // TODO: quickly restore previous hash.
    }
}

impl std::cmp::PartialEq for IndexPathKey {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash // much faster, and low chance of collision
    }
}

impl std::hash::Hash for IndexPathKey {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl nohash_hasher::IsEnabled for IndexPathKey {}

// ----------------------------------------------------------------------------

/// Hash the given value.
#[inline]
fn hash(value: impl std::hash::Hash) -> u64 {
    hash_with_seed(value, 456)
}

/// Hash the given value.
#[inline]
fn hash_with_seed(value: impl std::hash::Hash, seed: u128) -> u64 {
    use std::hash::Hasher as _;
    let mut hasher = ahash::AHasher::new_with_keys(123, seed);
    value.hash(&mut hasher);
    hasher.finish()
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimeValue {
    // Time(u64), // TODO
    Sequence(i64),
}

/// A point in time.
///
/// It can be represented by [`Time`], a sequence index, or a mix of several things.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeStamp(pub BTreeMap<String, TimeValue>);

#[derive(Clone, Copy, Debug)]
pub struct TimeRange {
    pub min: TimeValue,
    pub max: TimeValue,
}

pub enum TimeQuery {
    LatestAt(TimeValue),
    Range(TimeRange),
}
