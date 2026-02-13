use std::hash::Hash;
use std::str::FromStr;

use re_chunk::RowId;
use re_log_types::{DataPath, EntityPath, EntityPathHash, Instance, PathParseError};

use crate::{EntityDb, VersionedInstancePath, VersionedInstancePathHash};

// ----------------------------------------------------------------------------

/// The path to either a specific instance of an entity, or the whole entity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InstancePath {
    pub entity_path: EntityPath,

    /// If this is a concrete instance, what instance index are we?
    ///
    /// If we refer to all instances, [`Instance::ALL`] is used.
    pub instance: Instance,
}

impl From<EntityPath> for InstancePath {
    #[inline]
    fn from(entity_path: EntityPath) -> Self {
        Self::entity_all(entity_path)
    }
}

impl InstancePath {
    /// Indicate the whole entity (all instances of it).
    ///
    /// For example: the whole point cloud, rather than a specific point.
    #[inline]
    pub fn entity_all(entity_path: impl Into<EntityPath>) -> Self {
        Self {
            entity_path: entity_path.into(),
            instance: Instance::ALL,
        }
    }

    /// Indicate a specific instance of the entity,
    /// e.g. a specific point in a point cloud entity.
    #[inline]
    pub fn instance(entity_path: impl Into<EntityPath>, instance: impl Into<Instance>) -> Self {
        Self {
            entity_path: entity_path.into(),
            instance: instance.into(),
        }
    }

    /// Do we refer to the whole entity (all instances of it)?
    ///
    /// For example: the whole point cloud, rather than a specific point.
    #[inline]
    pub fn is_all(&self) -> bool {
        self.instance.is_all()
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
            instance: self.instance,
        }
    }

    /// Human-readable description of the kind
    pub fn kind(&self) -> &'static str {
        if self.instance.is_specific() {
            "Entity instance"
        } else {
            "Entity"
        }
    }
}

impl std::fmt::Display for InstancePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.instance.is_all() {
            self.entity_path.fmt(f)
        } else {
            format!("{}[{}]", self.entity_path, self.instance).fmt(f)
        }
    }
}

impl FromStr for InstancePath {
    type Err = PathParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let DataPath {
            entity_path,
            instance,
            component,
        } = DataPath::from_str(s)?;

        if let Some(component) = component {
            return Err(PathParseError::UnexpectedComponent(component));
        }

        let instance = instance.unwrap_or(Instance::ALL);

        Ok(Self {
            entity_path,
            instance,
        })
    }
}

#[test]
fn test_parse_instance_path() {
    assert_eq!(
        InstancePath::from_str("world/points[#123]"),
        Ok(InstancePath {
            entity_path: EntityPath::from("world/points"),
            instance: Instance::from(123)
        })
    );
}

// ----------------------------------------------------------------------------

/// Hashes of the components of an [`InstancePath`].
///
/// This is unique to either a specific instance of an entity, or the whole entity.
#[derive(Clone, Copy, Eq)]
pub struct InstancePathHash {
    pub entity_path_hash: EntityPathHash,

    /// If this is a concrete instance, what instance index are we?
    ///
    /// If we refer to all instance, [`Instance::ALL`] is used.
    ///
    /// Note that this is NOT hashed, because we don't need to (it's already small).
    pub instance: Instance,
}

impl std::fmt::Debug for InstancePathHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            entity_path_hash,
            instance,
        } = self;
        write!(
            f,
            "InstancePathHash({:016X}, {})",
            entity_path_hash.hash64(),
            instance.get()
        )
    }
}

impl std::hash::Hash for InstancePathHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Self {
            entity_path_hash,
            instance,
        } = self;

        state.write_u64(entity_path_hash.hash64());
        state.write_u64(instance.get());
    }
}

impl std::cmp::PartialEq for InstancePathHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let Self {
            entity_path_hash,
            instance,
        } = self;

        entity_path_hash == &other.entity_path_hash && instance == &other.instance
    }
}

impl InstancePathHash {
    pub const NONE: Self = Self {
        entity_path_hash: EntityPathHash::NONE,
        instance: Instance::ALL,
    };

    /// Indicate the whole entity (all instances of it).
    ///
    /// For example: the whole point cloud, rather than a specific point.
    #[inline]
    pub fn entity_all(entity_path: &EntityPath) -> Self {
        Self {
            entity_path_hash: entity_path.hash(),
            instance: Instance::ALL,
        }
    }

    /// Indicate a specific instance of the entity,
    /// e.g. a specific point in a point cloud entity.
    #[inline]
    pub fn instance(entity_path: &EntityPath, instance: Instance) -> Self {
        Self {
            entity_path_hash: entity_path.hash(),
            instance,
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

        let instance = self.instance;

        Some(InstancePath {
            entity_path,
            instance,
        })
    }
}

impl Default for InstancePathHash {
    fn default() -> Self {
        Self::NONE
    }
}
