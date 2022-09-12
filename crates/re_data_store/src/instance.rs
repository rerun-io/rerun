use std::hash::Hash;

use re_log_types::{Index, IndexHash, ObjPath, ObjPathHash};

use crate::{DataStore, InstanceProps};

// ----------------------------------------------------------------------------

/// A specific instance of a multi-object, or the (only) instance of a mono-object.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InstanceId {
    pub obj_path: ObjPath,

    /// If this is a multi-object, what instance index are we?
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

    /// Does this object match this instance id?
    #[inline]
    pub fn is_instance(&self, props: &InstanceProps<'_>) -> bool {
        &self.obj_path == props.obj_path
            && if let Some(index) = &self.instance_index {
                index.hash() == props.instance_index
            } else {
                props.instance_index.is_none()
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InstanceIdHash {
    pub obj_path_hash: ObjPathHash,

    /// If this is a multi-object, what instance index are we?
    /// [`IndexHash::NONE`] if we aren't a multi-object.
    pub instance_index_hash: IndexHash,
}

impl InstanceIdHash {
    pub const NONE: Self = Self {
        obj_path_hash: ObjPathHash::NONE,
        instance_index_hash: IndexHash::NONE,
    };

    #[inline]
    pub fn from_props(props: &InstanceProps<'_>) -> Self {
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

    /// Does this object match this instance id?
    #[inline]
    pub fn is_instance(&self, props: &InstanceProps<'_>) -> bool {
        &self.obj_path_hash == props.obj_path.hash()
            && self.instance_index_hash == props.instance_index
    }
}
