use std::hash::Hash;

use re_log_types::{field_types::Instance, Index, IndexHash, ObjPath, ObjPathHash};

use crate::log_db::ObjDb;

// ----------------------------------------------------------------------------

/// A specific instance of a multi-object, or the (only) instance of a mono-object.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InstanceId {
    pub obj_path: ObjPath,

    /// If this is a concrete instance, what instance index are we?
    pub instance_index: Option<Index>,
}

impl InstanceId {
    #[inline]
    pub fn new(obj_path: ObjPath, instance_index: Option<Index>) -> Self {
        Self {
            obj_path,
            instance_index,
        }
    }

    #[inline]
    pub fn hash(&self) -> InstanceIdHash {
        InstanceIdHash {
            obj_path_hash: self.obj_path.hash(),
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

    /// Does this object match this instance id?
    #[inline]
    pub fn is_instance(&self, obj_path: &ObjPath, instance_index: IndexHash) -> bool {
        &self.obj_path == obj_path
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
            format!("{}[{}]", self.obj_path, instance_index).fmt(f)
        } else {
            self.obj_path.fmt(f)
        }
    }
}

// ----------------------------------------------------------------------------

/// Hashes of the components of an [`InstanceId`].
#[derive(Clone, Copy, Debug, Eq)]
pub struct InstanceIdHash {
    pub obj_path_hash: ObjPathHash,

    /// If this is a multi-object, what instance index are we?
    /// [`IndexHash::NONE`] if we aren't a multi-object.
    pub instance_index_hash: IndexHash,

    /// If this is an arrow instance, hang onto the Instance
    /// TODO(jleibs): this can go way once we have an arrow-store resolver
    pub arrow_instance: Option<re_log_types::field_types::Instance>,
}

impl std::hash::Hash for InstanceIdHash {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.obj_path_hash.hash64());
        state.write_u64(self.instance_index_hash.hash64());
    }
}

impl std::cmp::PartialEq for InstanceIdHash {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.obj_path_hash == other.obj_path_hash
            && self.instance_index_hash == other.instance_index_hash
    }
}

impl InstanceIdHash {
    pub const NONE: Self = Self {
        obj_path_hash: ObjPathHash::NONE,
        instance_index_hash: IndexHash::NONE,
        arrow_instance: None,
    };

    #[inline]
    pub fn from_path_and_index(obj_path: &ObjPath, instance_index: IndexHash) -> Self {
        Self {
            obj_path_hash: obj_path.hash(),
            instance_index_hash: instance_index,
            arrow_instance: None,
        }
    }

    #[inline]
    pub fn from_path_and_arrow_instance(obj_path: &ObjPath, arrow_instance: &Instance) -> Self {
        Self {
            obj_path_hash: obj_path.hash(),
            instance_index_hash: Index::ArrowInstance(*arrow_instance).hash(),
            arrow_instance: Some(*arrow_instance),
        }
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        self.obj_path_hash.is_some()
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        self.obj_path_hash.is_none()
    }

    pub fn resolve(&self, obj_db: &ObjDb) -> Option<InstanceId> {
        match self.arrow_instance {
            None => {
                re_log::error_once!("Found classical InstanceIdHash");
                None
            }
            Some(arrow_instance) => Some(InstanceId {
                obj_path: obj_db.obj_path_from_hash(&self.obj_path_hash).cloned()?,
                instance_index: Some(Index::ArrowInstance(arrow_instance)),
            }),
        }
    }

    /// Does this object match this instance id?
    #[inline]
    pub fn is_instance(&self, obj_path: &ObjPath, instance_index: IndexHash) -> bool {
        self.obj_path_hash == obj_path.hash() && self.instance_index_hash == instance_index
    }
}

impl Default for InstanceIdHash {
    fn default() -> Self {
        Self::NONE
    }
}
