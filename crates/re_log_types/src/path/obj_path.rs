use crate::{
    hash::Hash128,
    path::{obj_path_impl::ObjPathImpl, IndexPath, ObjTypePath},
    Index, ObjPathComp,
};

// ----------------------------------------------------------------------------

/// A 128 bit hash of [`ObjPath`] with negligible chance of collision.
#[derive(Copy, Clone, Eq)]
pub struct ObjPathHash(Hash128);

impl ObjPathHash {
    /// Sometimes used as the hash of `None`.
    pub const NONE: ObjPathHash = ObjPathHash(Hash128::ZERO);

    #[inline]
    pub fn hash64(&self) -> u64 {
        self.0.hash64()
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        *self != Self::NONE
    }
}

impl std::hash::Hash for ObjPathHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.hash64());
    }
}

impl std::cmp::PartialEq for ObjPathHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl nohash_hasher::IsEnabled for ObjPathHash {}

impl std::fmt::Debug for ObjPathHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "ObjPathHash({:016X}{:016X})",
            self.0.first64(),
            self.0.second64()
        ))
    }
}

// ----------------------------------------------------------------------------

/// `camera / "left" / points / #42`
#[derive(Clone, Debug, Eq)]
pub struct ObjPath {
    /// precomputed hash
    hash: ObjPathHash,

    // boxed to keep down the size of [`ObjPath`].
    // We mostly use the hash for lookups and comparisons anyway!
    path: Box<ObjPathImpl>,
}

impl ObjPath {
    #[inline]
    pub fn root() -> Self {
        Self::from(ObjPathImpl::root())
    }

    #[inline]
    pub fn new(obj_type_path: ObjTypePath, index_path: IndexPath) -> Self {
        Self::from(ObjPathImpl::new(obj_type_path, index_path))
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        self.path.iter()
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.path.is_root()
    }

    #[inline]
    pub fn hash(&self) -> &ObjPathHash {
        &self.hash
    }

    /// Precomputed 64-bit hash.
    #[inline]
    pub fn hash64(&self) -> u64 {
        self.hash.hash64()
    }

    #[inline]
    pub fn obj_type_path(&self) -> &ObjTypePath {
        self.path.obj_type_path()
    }

    #[inline]
    pub fn index_path(&self) -> &IndexPath {
        self.path.index_path()
    }

    pub fn to_components(&self) -> Vec<ObjPathComp> {
        self.path.to_components()
    }

    #[inline]
    pub fn into_type_path_and_index_path(self) -> (ObjTypePath, IndexPath) {
        self.path.into_type_path_and_index_path()
    }

    #[must_use]
    pub fn parent(&self) -> Self {
        let parent = self.path.parent();
        Self::from(parent)
    }

    /// Replace last [`Index::Placeholder`] with the given key.
    #[must_use]
    pub fn replace_last_placeholder_with(self, key: Index) -> Self {
        Self::from(self.path.replace_last_placeholder_with(key))
    }
}

impl From<ObjPathImpl> for ObjPath {
    #[inline]
    fn from(path: ObjPathImpl) -> Self {
        Self {
            hash: ObjPathHash(Hash128::hash(&path)),
            path: Box::new(path),
        }
    }
}

impl From<Vec<ObjPathComp>> for ObjPath {
    #[inline]
    fn from(path: Vec<ObjPathComp>) -> Self {
        Self::from(ObjPathImpl::from(path.iter()))
    }
}

impl From<&[ObjPathComp]> for ObjPath {
    #[inline]
    fn from(path: &[ObjPathComp]) -> Self {
        Self::from(ObjPathImpl::from(path.iter()))
    }
}

impl From<&str> for ObjPath {
    #[inline]
    fn from(component: &str) -> Self {
        Self::from(vec![ObjPathComp::from(component)])
    }
}

impl From<String> for ObjPath {
    #[inline]
    fn from(component: String) -> Self {
        Self::from(vec![ObjPathComp::from(component)])
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "serde")]
impl serde::Serialize for ObjPath {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.path.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ObjPath {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        ObjPathImpl::deserialize(deserializer).map(Self::from)
    }
}

// ----------------------------------------------------------------------------

impl std::cmp::PartialEq for ObjPath {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash // much faster, and low chance of collision
    }
}

impl std::hash::Hash for ObjPath {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl nohash_hasher::IsEnabled for ObjPath {}

// ----------------------------------------------------------------------------

impl std::cmp::Ord for ObjPath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.path.cmp(&other.path)
    }
}

impl std::cmp::PartialOrd for ObjPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.path.cmp(&other.path))
    }
}

// ----------------------------------------------------------------------------

pub use super::obj_path_impl::Iter;
pub use super::obj_path_impl::ObjPathComponentRef;

// ----------------------------------------------------------------------------

impl std::fmt::Display for ObjPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.fmt(f)
    }
}
