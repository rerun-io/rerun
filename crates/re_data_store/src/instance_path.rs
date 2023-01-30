use std::hash::Hash;

use re_log_types::{component_types::Instance, EntityPath, EntityPathHash};

use crate::log_db::EntityDb;

// ----------------------------------------------------------------------------

/// The path to either a specific instance of an entity, or the whole entity (splat).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InstancePath {
    pub entity_path: EntityPath,

    /// If this is a concrete instance, what instance index are we?
    ///
    /// If we refer to all instance, [`Instance::SPLAT`] is used.
    pub instance_index: Instance,
}

impl InstancePath {
    /// Indicate the whole entity (all instances of it) - i.e. a splat.
    ///
    /// For instance: the whole point cloud, rather than a specific point.
    #[inline]
    pub fn entity_splat(entity_path: EntityPath) -> Self {
        Self {
            entity_path,
            instance_index: Instance::SPLAT,
        }
    }

    /// Indicate a specific instance of the entity,
    /// e.g. a specific point in a point cloud entity.
    #[inline]
    pub fn instance(entity_path: EntityPath, instance_index: Instance) -> Self {
        Self {
            entity_path,
            instance_index,
        }
    }

    /// Do we refer to the whole entity (all instances of it)?
    ///
    /// For instance: the whole point cloud, rather than a specific point.
    #[inline]
    pub fn is_splat(&self) -> bool {
        self.instance_index.is_splat()
    }

    #[inline]
    pub fn hash(&self) -> InstancePathHash {
        InstancePathHash {
            entity_path_hash: self.entity_path.hash(),
            instance_index: self.instance_index,
        }
    }

    /// Does this entity match this instance id?
    #[inline]
    pub fn is_instance(&self, entity_path: &EntityPath, instance_index: Instance) -> bool {
        &self.entity_path == entity_path && self.instance_index == instance_index
    }
}

impl std::fmt::Display for InstancePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.instance_index.is_splat() {
            self.entity_path.fmt(f)
        } else {
            format!("{}[{}]", self.entity_path, self.instance_index).fmt(f)
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
    /// If we refer to all instance, [`Instance::SPLAT`] is used.
    ///
    /// Note that this is NOT hashed, because we don't need to (it's already small).
    pub instance_index: Instance,
}

impl std::hash::Hash for InstancePathHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.entity_path_hash.hash64());
        state.write_u64(self.instance_index.0);
    }
}

impl std::cmp::PartialEq for InstancePathHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.entity_path_hash == other.entity_path_hash
            && self.instance_index == other.instance_index
    }
}

impl InstancePathHash {
    pub const NONE: Self = Self {
        entity_path_hash: EntityPathHash::NONE,
        instance_index: Instance::SPLAT,
    };

    /// Indicate the whole entity (all instances of it) - i.e. a splat.
    ///
    /// For instance: the whole point cloud, rather than a specific point.
    #[inline]
    pub fn entity_splat(entity_path: &EntityPath) -> Self {
        Self {
            entity_path_hash: entity_path.hash(),
            instance_index: Instance::SPLAT,
        }
    }

    /// Indicate a specific instance of the entity,
    /// e.g. a specific point in a point cloud entity.
    #[inline]
    pub fn instance(entity_path: &EntityPath, instance_index: Instance) -> Self {
        Self {
            entity_path_hash: entity_path.hash(),
            instance_index,
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

    pub fn resolve(&self, entity_db: &EntityDb) -> Option<InstancePath> {
        let entity_path = entity_db
            .entity_path_from_hash(&self.entity_path_hash)
            .cloned()?;

        let instance_index = self.instance_index;

        Some(InstancePath {
            entity_path,
            instance_index,
        })
    }

    /// Does this entity match this instance id?
    #[inline]
    pub fn is_instance(&self, entity_path: &EntityPath, instance_index: Instance) -> bool {
        self.entity_path_hash == entity_path.hash() && self.instance_index == instance_index
    }
}

impl Default for InstancePathHash {
    fn default() -> Self {
        Self::NONE
    }
}
