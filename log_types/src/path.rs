mod data_path;
mod index_path;
mod obj_path;
mod obj_path_builder;
mod obj_type_path;

pub use data_path::DataPath;
pub use index_path::{IndexKey, IndexPath};
pub use obj_path::ObjPath;
pub use obj_path_builder::ObjPathBuilder;
pub use obj_type_path::ObjTypePath;

use rr_string_interner::InternedString;

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

/// The key of a table.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Index {
    /// For arrays, assumed to be dense (0, 1, 2, …).
    Sequence(u64),

    /// X,Y pixel coordinates, from top left.
    Pixel([u64; 2]),

    /// Any integer, e.g. a hash or an arbitrary identifier.
    Integer(i128),

    /// UUID/GUID
    Uuid(uuid::Uuid),

    /// Anything goes.
    String(String),

    /// Used as the last index when logging a batch of data.
    Placeholder, // TODO: `ObjPathComp::IndexPlaceholder` instead?
}

impl std::fmt::Display for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequence(seq) => format!("#{seq}").fmt(f),
            Self::Pixel([x, y]) => format!("[{x}, {y}]").fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::Uuid(value) => value.fmt(f),
            Self::String(value) => format!("{value:?}").fmt(f), // put it in quotes
            Self::Placeholder => '_'.fmt(f),                    // put it in quotes
        }
    }
}

crate::impl_into_enum!(String, Index, String);

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
