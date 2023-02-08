use std::sync::Arc;

use crate::{
    hash::Hash128, parse_entity_path, path::entity_path_impl::EntityPathImpl, EntityPathPart,
};

// ----------------------------------------------------------------------------

/// A 128 bit hash of [`EntityPath`] with negligible risk of collision.
#[derive(Copy, Clone, Eq)]
pub struct EntityPathHash(Hash128);

impl EntityPathHash {
    /// Sometimes used as the hash of `None`.
    pub const NONE: EntityPathHash = EntityPathHash(Hash128::ZERO);

    #[inline]
    pub fn hash64(&self) -> u64 {
        self.0.hash64()
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        *self != Self::NONE
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        *self == Self::NONE
    }
}

impl std::hash::Hash for EntityPathHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.hash64());
    }
}

impl std::cmp::PartialEq for EntityPathHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl nohash_hasher::IsEnabled for EntityPathHash {}

impl std::fmt::Debug for EntityPathHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "EntityPathHash({:016X}{:016X})",
            self.0.first64(),
            self.0.second64()
        ))
    }
}

// ----------------------------------------------------------------------------

/// `camera / "left" / points / #42`
///
/// Cheap to clone.
///
/// Implements [`nohash_hasher::IsEnabled`].
#[derive(Clone, Eq)]
pub struct EntityPath {
    /// precomputed hash
    hash: EntityPathHash,

    // [`Arc`] used for cheap cloning, and to keep down the size of [`EntityPath`].
    // We mostly use the hash for lookups and comparisons anyway!
    path: Arc<EntityPathImpl>,
}

impl EntityPath {
    #[inline]
    pub fn root() -> Self {
        Self::from(EntityPathImpl::root())
    }

    #[inline]
    pub fn new(parts: Vec<EntityPathPart>) -> Self {
        Self::from(parts)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &EntityPathPart> {
        self.path.iter()
    }

    pub fn last(&self) -> Option<&EntityPathPart> {
        self.path.last()
    }

    #[inline]
    pub fn as_slice(&self) -> &[EntityPathPart] {
        self.path.as_slice()
    }

    #[inline]
    pub fn is_root(&self) -> bool {
        self.path.is_root()
    }

    /// Is this a strict descendant of the given path.
    #[inline]
    pub fn is_descendant_of(&self, other: &EntityPath) -> bool {
        other.len() < self.len() && self.path.iter().zip(other.iter()).all(|(a, b)| a == b)
    }

    /// Is this a direct child of the other path.
    #[inline]
    pub fn is_child_of(&self, other: &EntityPath) -> bool {
        other.len() + 1 == self.len() && self.path.iter().zip(other.iter()).all(|(a, b)| a == b)
    }

    /// Number of parts
    #[inline]
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.path.len()
    }

    #[inline]
    pub fn hash(&self) -> EntityPathHash {
        self.hash
    }

    /// Precomputed 64-bit hash.
    #[inline]
    pub fn hash64(&self) -> u64 {
        self.hash.hash64()
    }

    /// Return [`None`] if root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        self.path.parent().map(Self::from)
    }

    pub fn join(&self, other: &Self) -> Self {
        self.iter().chain(other.iter()).cloned().collect()
    }
}

impl FromIterator<EntityPathPart> for EntityPath {
    fn from_iter<T: IntoIterator<Item = EntityPathPart>>(parts: T) -> Self {
        Self::new(parts.into_iter().collect())
    }
}

impl From<EntityPathImpl> for EntityPath {
    #[inline]
    fn from(path: EntityPathImpl) -> Self {
        Self {
            hash: EntityPathHash(Hash128::hash(&path)),
            path: Arc::new(path),
        }
    }
}

impl From<Vec<EntityPathPart>> for EntityPath {
    #[inline]
    fn from(path: Vec<EntityPathPart>) -> Self {
        Self::from(EntityPathImpl::from(path.iter()))
    }
}

impl From<&[EntityPathPart]> for EntityPath {
    #[inline]
    fn from(path: &[EntityPathPart]) -> Self {
        Self::from(EntityPathImpl::from(path.iter()))
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<&str> for EntityPath {
    #[inline]
    fn from(path: &str) -> Self {
        Self::from(parse_entity_path(path).unwrap())
    }
}

impl From<String> for EntityPath {
    #[inline]
    fn from(path: String) -> Self {
        Self::from(path.as_str())
    }
}

impl From<EntityPath> for String {
    #[inline]
    fn from(path: EntityPath) -> Self {
        path.to_string()
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "serde")]
impl serde::Serialize for EntityPath {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.path.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for EntityPath {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        EntityPathImpl::deserialize(deserializer).map(Self::from)
    }
}

// ----------------------------------------------------------------------------

impl std::cmp::PartialEq for EntityPath {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash // much faster, and low risk of collision
    }
}

impl std::hash::Hash for EntityPath {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl nohash_hasher::IsEnabled for EntityPath {}

// ----------------------------------------------------------------------------

impl std::cmp::Ord for EntityPath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.path.cmp(&other.path)
    }
}

impl std::cmp::PartialOrd for EntityPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.path.cmp(&other.path))
    }
}

// ----------------------------------------------------------------------------

impl std::fmt::Debug for EntityPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.fmt(f)
    }
}

impl std::fmt::Display for EntityPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.fmt(f)
    }
}
