use std::hash::Hash;

use re_log_types::{Index, IndexHash, ObjPath, ObjPathHash};

use crate::{DataStore, ObjectProps};

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InstanceId {
    pub obj_path: ObjPath,

    /// If this is a multi-object, what index are we?
    pub instance_index: Option<Index>,
}

impl InstanceId {
    #[inline]
    pub fn hash(&self) -> InstanceIdHash {
        InstanceIdHash {
            obj_path_hash: *self.obj_path.hash(),
            instance_index_hash: self.instance_index_hash(),
        }
    }

    #[inline]
    pub fn is_obj_props(&self, obj_props: &ObjectProps<'_>) -> bool {
        &self.obj_path == obj_props.obj_path
            && if let Some(index) = &self.instance_index {
                index.hash() == obj_props.instance_index
            } else {
                obj_props.instance_index.is_none()
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InstanceIdHash {
    pub obj_path_hash: ObjPathHash,
    /// If this is a multi-object, what index are we?
    pub instance_index_hash: IndexHash,
}

impl InstanceIdHash {
    pub const NONE: Self = Self {
        obj_path_hash: ObjPathHash::NONE,
        instance_index_hash: IndexHash::NONE,
    };

    #[inline]
    pub fn from_props(props: &ObjectProps<'_>) -> Self {
        Self {
            obj_path_hash: *props.obj_path.hash(),
            instance_index_hash: props.instance_index,
        }
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        self.obj_path_hash.is_some()
    }

    pub fn resolve(&self, store: &DataStore) -> Option<InstanceId> {
        Some(InstanceId {
            obj_path: store.obj_path_from_hash(&self.obj_path_hash).cloned()?,
            instance_index: store.index_from_hash(&self.instance_index_hash).cloned(),
        })
    }

    #[inline]
    pub fn is_obj_props(&self, obj_props: &ObjectProps<'_>) -> bool {
        &self.obj_path_hash == obj_props.obj_path.hash()
            && self.instance_index_hash == obj_props.instance_index
    }
}
