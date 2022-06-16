mod data_path;
mod index_path;
mod obj_path;
mod obj_path_builder;
pub(crate) mod obj_path_impl;
mod obj_type_path;

pub use data_path::DataPath;
pub use index_path::{IndexPath, IndexPathHash};
pub use obj_path::{ObjPath, ObjPathHash};
pub use obj_path_builder::ObjPathBuilder;
pub use obj_type_path::ObjTypePath;

use rr_string_interner::InternedString;

use crate::Index;

rr_string_interner::declare_new_type!(
    /// The name of a object field, e.g. "pos" or "color".
    pub struct FieldName;
);

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ObjPathComp {
    /// Struct member. Each member can have a different type.
    String(InternedString),

    /// Array/table/map member. Each member must be of the same type (homogenous).
    Index(Index),
}

impl ObjPathComp {
    pub fn to_type_path_comp(&self) -> TypePathComp {
        match self {
            Self::String(name) => TypePathComp::String(*name),
            Self::Index(_) => TypePathComp::Index,
        }
    }
}

impl std::fmt::Display for ObjPathComp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(string) => f.write_str(string),
            Self::Index(index) => index.fmt(f),
        }
    }
}

impl From<&str> for ObjPathComp {
    #[inline]
    fn from(comp: &str) -> Self {
        Self::String(comp.into())
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TypePathComp {
    /// Struct member
    String(InternedString),

    /// Table (array/map) member.
    /// Tables are homogenous, so it is the same type path for all.
    Index,
}

impl std::fmt::Display for TypePathComp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(string) => string.fmt(f),
            Self::Index => '*'.fmt(f),
        }
    }
}
