use std::hash::Hash;

use re_chunk::RowId;

use crate::{InstancePath, InstancePathHash};

// ----------------------------------------------------------------------------

/// A versioned path (i.e. pinned to a specific [`RowId`]) to either a specific instance of an entity,
/// or the whole entity.
///
/// The easiest way to construct this type is via [`crate::InstancePath::versioned`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct VersionedInstancePath {
    pub instance_path: InstancePath,
    pub row_id: RowId,
}

impl VersionedInstancePath {
    /// Do we refer to the whole entity (all instances of it)?
    ///
    /// For example: the whole point cloud, rather than a specific point.
    #[inline]
    pub fn is_all(&self) -> bool {
        self.instance_path.is_all()
    }

    #[inline]
    pub fn hash(&self) -> VersionedInstancePathHash {
        VersionedInstancePathHash {
            instance_path_hash: self.instance_path.hash(),
            row_id: self.row_id,
        }
    }
}

impl std::fmt::Display for VersionedInstancePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        format!("{} @ {}", self.instance_path, self.row_id).fmt(f)
    }
}

// ----------------------------------------------------------------------------

/// Hashes of the components of a [`VersionedInstancePath`].
///
/// The easiest way to construct this type is to use either [`crate::InstancePathHash::versioned`]
/// or [`crate::VersionedInstancePath::hash`].
#[derive(Clone, Copy, Eq)]
pub struct VersionedInstancePathHash {
    pub instance_path_hash: InstancePathHash,
    pub row_id: RowId,
}

impl re_byte_size::SizeBytes for VersionedInstancePathHash {
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            instance_path_hash: _,
            row_id: _,
        } = self;
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl std::fmt::Debug for VersionedInstancePathHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            instance_path_hash,
            row_id,
        } = self;
        write!(
            f,
            "VersionedInstancePathHash({instance_path_hash:?}, {row_id})"
        )
    }
}

impl std::hash::Hash for VersionedInstancePathHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Self {
            instance_path_hash,
            row_id,
        } = self;
        let InstancePathHash {
            entity_path_hash,
            instance,
        } = instance_path_hash;

        state.write_u64(entity_path_hash.hash64());
        state.write_u64(instance.get());
        state.write_u128(row_id.as_u128());
    }
}

impl std::cmp::PartialEq for VersionedInstancePathHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let Self {
            instance_path_hash,
            row_id,
        } = self;

        instance_path_hash == &other.instance_path_hash && row_id == &other.row_id
    }
}

impl VersionedInstancePathHash {
    pub const NONE: Self = Self {
        instance_path_hash: InstancePathHash::NONE,
        row_id: RowId::ZERO,
    };

    #[inline]
    pub fn is_some(&self) -> bool {
        self.instance_path_hash.is_some()
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.instance_path_hash.is_none()
    }
}

impl Default for VersionedInstancePathHash {
    fn default() -> Self {
        Self::NONE
    }
}
