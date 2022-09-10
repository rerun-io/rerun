use re_log_types::{Index, IndexHash, ObjPath, ObjPathHash};

use crate::{LogDataStore, ObjectProps};

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct InstanceId {
    pub obj_path: ObjPath,

    /// If this is a multi-object, what index are we?
    pub multi_index: Option<Index>,
}

impl InstanceId {
    pub fn hash(&self) -> InstanceIdHash {
        InstanceIdHash {
            obj_path_hash: *self.obj_path.hash(),
            multi_index_hash: self.multi_index_hash(),
        }
    }

    pub fn is_obj_props(&self, obj_props: &ObjectProps<'_>) -> bool {
        &self.obj_path == obj_props.obj_path
            && match (&self.multi_index, obj_props.multi_index) {
                (Some(_), None) | (None, Some(_)) => false,
                (None, None) => true,
                (Some(a), Some(b)) => &a.hash() == b,
            }
    }

    pub fn multi_index_hash(&self) -> IndexHash {
        self.multi_index
            .as_ref()
            .map_or(IndexHash::NONE, Index::hash)
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(multi_index) = &self.multi_index {
            format!("{}[{}]", self.obj_path, multi_index).fmt(f)
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
    pub multi_index_hash: IndexHash,
}

impl InstanceIdHash {
    pub const NONE: Self = Self {
        obj_path_hash: ObjPathHash::NONE,
        multi_index_hash: IndexHash::NONE,
    };

    pub fn from_props(props: &ObjectProps<'_>) -> Self {
        Self {
            obj_path_hash: *props.obj_path.hash(),
            multi_index_hash: props.multi_index.copied().unwrap_or(IndexHash::NONE),
        }
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        self.obj_path_hash.is_some()
    }

    pub fn resolve(&self, store: &LogDataStore) -> Option<InstanceId> {
        Some(InstanceId {
            obj_path: store.obj_path_from_hash(&self.obj_path_hash).cloned()?,
            multi_index: store.index_from_hash(&self.multi_index_hash).cloned(),
        })
    }
}
