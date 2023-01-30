use std::hash::Hash;

use re_log_types::{field_types::Instance, EntityPath, EntityPathHash, Index, IndexHash};

use crate::log_db::EntityDb;

// ----------------------------------------------------------------------------

/// Either a specific instance of an entity, or the whole entity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InstanceId {
    pub entity_path: EntityPath,

    /// If this is a concrete instance, what instance index are we?
    pub instance_index: Option<Index>,
}

impl InstanceId {
    #[inline]
    pub fn new(entity_path: EntityPath, instance_index: Option<Index>) -> Self {
        Self {
            entity_path,
            instance_index,
        }
    }

    #[inline]
    pub fn hash(&self) -> InstanceIdHash {
        InstanceIdHash {
            entity_path_hash: self.entity_path.hash(),
            instance_index_hash: self.instance_index_hash(),
            arrow_instance: {
                if let Some(Index::ArrowInstance(key)) = &self.instance_index {
                    Some(*key)
                } else {
                    None
                }
            },
        }
    }

    /// Does this entity match this instance id?
    #[inline]
    pub fn is_instance(&self, entity_path: &EntityPath, instance_index: IndexHash) -> bool {
        &self.entity_path == entity_path
            && if let Some(index) = &self.instance_index {
                index.hash() == instance_index
            } else {
                instance_index.is_none()
            }
    }

    pub fn instance_index_hash(&self) -> IndexHash {
        self.instance_index
            .as_ref()
            .map_or(IndexHash::NONE, Index::hash)
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(instance_index) = &self.instance_index {
            format!("{}[{}]", self.entity_path, instance_index).fmt(f)
        } else {
            self.entity_path.fmt(f)
        }
    }
}

// ----------------------------------------------------------------------------

/// Hashes of the components of an [`InstanceId`].
#[derive(Clone, Copy, Debug, Eq)]
pub struct InstanceIdHash {
    pub entity_path_hash: EntityPathHash,

    /// If this is a multi-entity, what instance index are we?
    /// [`IndexHash::NONE`] if we aren't a multi-entity.
    pub instance_index_hash: IndexHash,

    /// If this is an arrow instance, hang onto the Instance
    /// TODO(jleibs): this can go way once we have an arrow-store resolver
    pub arrow_instance: Option<re_log_types::field_types::Instance>,
}

impl std::hash::Hash for InstanceIdHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.entity_path_hash.hash64());
        state.write_u64(self.instance_index_hash.hash64());
    }
}

impl std::cmp::PartialEq for InstanceIdHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.entity_path_hash == other.entity_path_hash
            && self.instance_index_hash == other.instance_index_hash
    }
}

impl InstanceIdHash {
    pub const NONE: Self = Self {
        entity_path_hash: EntityPathHash::NONE,
        instance_index_hash: IndexHash::NONE,
        arrow_instance: None,
    };

    #[inline]
    pub fn from_path_and_index(entity_path: &EntityPath, instance_index: IndexHash) -> Self {
        Self {
            entity_path_hash: entity_path.hash(),
            instance_index_hash: instance_index,
            arrow_instance: None,
        }
    }

    #[inline]
    pub fn from_path_and_arrow_instance(
        entity_path: &EntityPath,
        arrow_instance: &Instance,
    ) -> Self {
        Self {
            entity_path_hash: entity_path.hash(),
            instance_index_hash: Index::ArrowInstance(*arrow_instance).hash(),
            arrow_instance: Some(*arrow_instance),
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

    pub fn resolve(&self, entity_db: &EntityDb) -> Option<InstanceId> {
        match self.arrow_instance {
            None => {
                re_log::error_once!("Found classical InstanceIdHash");
                None
            }
            Some(arrow_instance) => Some(InstanceId {
                entity_path: entity_db
                    .entity_path_from_hash(&self.entity_path_hash)
                    .cloned()?,
                instance_index: Some(Index::ArrowInstance(arrow_instance)),
            }),
        }
    }

    /// Does this entity match this instance id?
    #[inline]
    pub fn is_instance(&self, entity_path: &EntityPath, instance_index: IndexHash) -> bool {
        self.entity_path_hash == entity_path.hash() && self.instance_index_hash == instance_index
    }
}

impl Default for InstanceIdHash {
    fn default() -> Self {
        Self::NONE
    }
}
