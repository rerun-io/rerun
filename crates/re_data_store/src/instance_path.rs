use std::hash::Hash;

use re_log_types::{EntityPath, EntityPathHash, RowId};
use re_types::components::InstanceKey;

use crate::{store_db::EntityDb, VersionedInstancePath, VersionedInstancePathHash};

// ----------------------------------------------------------------------------

/// The path to either a specific instance of an entity, or the whole entity (splat).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InstancePath {
    pub entity_path: EntityPath,

    /// If this is a concrete instance, what instance index are we?
    ///
    /// If we refer to all instance, [`InstanceKey::SPLAT`] is used.
    pub instance_key: InstanceKey,
}

impl InstancePath {
    /// Indicate the whole entity (all instances of it) - i.e. a splat.
    ///
    /// For example: the whole point cloud, rather than a specific point.
    #[inline]
    pub fn entity_splat(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            instance_key: InstanceKey::SPLAT,
        }
    }

    /// Indicate a specific instance of the entity,
    /// e.g. a specific point in a point cloud entity.
    #[inline]
    pub fn instance(entity_path: EntityPath, instance_key: InstanceKey) -> Self {
        Self {
            entity_path,
            instance_key,
        }
    }

    /// Do we refer to the whole entity (all instances of it)?
    ///
    /// For example: the whole point cloud, rather than a specific point.
    #[inline]
    pub fn is_splat(&self) -> bool {
        self.instance_key.is_splat()
    }

    /// Versions this instance path by stamping it with the specified [`RowId`].
    #[inline]
    pub fn versioned(&self, row_id: RowId) -> VersionedInstancePath {
        VersionedInstancePath {
            instance_path: self.clone(),
            row_id,
        }
    }

    #[inline]
    pub fn hash(&self) -> InstancePathHash {
        InstancePathHash {
            entity_path_hash: self.entity_path.hash(),
            instance_key: self.instance_key,
        }
    }
}

impl std::fmt::Display for InstancePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.instance_key.is_splat() {
            self.entity_path.fmt(f)
        } else {
            format!("{}[{}]", self.entity_path, self.instance_key).fmt(f)
        }
    }
}

// ----------------------------------------------------------------------------

/// Hashes of the components of an [`InstancePath`].
///
/// This is unique to either a specific instance of an entity, or the whole entity (splat).
#[derive(Clone, Copy, Debug, Eq)]
pub struct InstancePathHash {
    pub entity_path_hash: EntityPathHash,

    /// If this is a concrete instance, what instance index are we?
    ///
    /// If we refer to all instance, [`InstanceKey::SPLAT`] is used.
    ///
    /// Note that this is NOT hashed, because we don't need to (it's already small).
    pub instance_key: InstanceKey,
}

impl std::hash::Hash for InstancePathHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Self {
            entity_path_hash,
            instance_key,
        } = self;

        state.write_u64(entity_path_hash.hash64());
        state.write_u64(instance_key.0);
    }
}

impl std::cmp::PartialEq for InstancePathHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let Self {
            entity_path_hash,
            instance_key,
        } = self;

        entity_path_hash == &other.entity_path_hash && instance_key == &other.instance_key
    }
}

impl InstancePathHash {
    pub const NONE: Self = Self {
        entity_path_hash: EntityPathHash::NONE,
        instance_key: InstanceKey::SPLAT,
    };

    /// Indicate the whole entity (all instances of it) - i.e. a splat.
    ///
    /// For example: the whole point cloud, rather than a specific point.
    #[inline]
    pub fn entity_splat(entity_path: &EntityPath) -> Self {
        Self {
            entity_path_hash: entity_path.hash(),
            instance_key: InstanceKey::SPLAT,
        }
    }

    /// Indicate a specific instance of the entity,
    /// e.g. a specific point in a point cloud entity.
    #[inline]
    pub fn instance(entity_path: &EntityPath, instance_key: InstanceKey) -> Self {
        Self {
            entity_path_hash: entity_path.hash(),
            instance_key,
        }
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        self.entity_path_hash.is_some()
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.entity_path_hash.is_none()
    }

    /// Versions this hashed instance path by stamping it with the specified [`RowId`].
    #[inline]
    pub fn versioned(&self, row_id: RowId) -> VersionedInstancePathHash {
        VersionedInstancePathHash {
            instance_path_hash: *self,
            row_id,
        }
    }

    pub fn resolve(&self, entity_db: &EntityDb) -> Option<InstancePath> {
        let entity_path = entity_db
            .entity_path_from_hash(&self.entity_path_hash)
            .cloned()?;

        let instance_key = self.instance_key;

        Some(InstancePath {
            entity_path,
            instance_key,
        })
    }
}

impl Default for InstancePathHash {
    fn default() -> Self {
        Self::NONE
    }
}
