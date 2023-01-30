//! Every logged object in Rerun is logged to a [`ObjPath`].
//!
//! The path is made up out of several [`ObjPathComp`],
//! each of which is either a name ([`ObjPathComp::Name`])
//! or an [`Index`].
//!
//! The [`Index`]es are for tables, arrays etc.
//! You can split an [`ObjPath`] into the names and the indices,
//! and then you get a [`ObjTypePath`] and an [`IndexPath`], like so:
//!
//! * [`ObjPath`]:     `camera / "left" / points / #42`
//! * [`ObjTypePath`]: `camera / *      / points / *`
//! * [`IndexPath`]:   `       / "left" /       / #42`

mod data_path;
mod index_path;
mod obj_path;
mod obj_path_impl;
mod obj_type_path;
mod parse_path;

pub use data_path::{DataPath, FieldOrComponent};
pub use index_path::{IndexPath, IndexPathHash};
pub use obj_path::{ObjPath, ObjPathHash};
pub use obj_path_impl::{ObjPathCompRef, ObjPathImpl};
pub use obj_type_path::ObjTypePath;
pub use parse_path::{parse_obj_path, PathParseError};

use re_string_interner::InternedString;

use crate::Index;

re_string_interner::declare_new_type!(
    /// The name of an object field, e.g. `pos` or `color`.
    pub struct FieldName;
);

// ----------------------------------------------------------------------------

/// The different parts that make up an [`ObjPath`].
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ObjPathComp {
    /// Struct member. Each member can have a different type.
    Name(InternedString),

    /// Array/table/map member. Each member must be of the same type (homogenous).
    Index(Index),
}

impl ObjPathComp {
    pub fn to_type_path_comp(&self) -> ObjTypePathComp {
        match self {
            Self::Name(name) => ObjTypePathComp::Name(*name),
            Self::Index(_) => ObjTypePathComp::Index,
        }
    }
}

impl std::fmt::Display for ObjPathComp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(name) => f.write_str(name),
            Self::Index(index) => index.fmt(f),
        }
    }
}

impl From<&str> for ObjPathComp {
    #[inline]
    fn from(comp: &str) -> Self {
        Self::Name(comp.into())
    }
}

impl From<String> for ObjPathComp {
    #[inline]
    fn from(comp: String) -> Self {
        Self::Name(comp.into())
    }
}

impl From<Index> for ObjPathComp {
    #[inline]
    fn from(comp: Index) -> Self {
        Self::Index(comp)
    }
}

// ----------------------------------------------------------------------------

/// The different parts that make up a [`ObjTypePath`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ObjTypePathComp {
    /// Struct member
    Name(InternedString),

    /// Table (array/map) member.
    /// Tables are homogenous, so it is the same type path for all.
    Index,
}

impl std::fmt::Display for ObjTypePathComp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(name) => name.fmt(f),
            Self::Index => '*'.fmt(f),
        }
    }
}

// ----------------------------------------------------------------------------

/// Build a `Vec<ObjPathComp>`:
/// ```
/// # use re_log_types::*;
/// obj_path_vec!("foo", Index::Sequence(123));
/// ```
#[macro_export]
macro_rules! obj_path_vec {
        () => {
            vec![]
        };
        ($($comp: expr),* $(,)?) => {
            vec![ $($crate::ObjPathComp::from($comp),)+ ]
        };
    }

/// Build a `ObjPath`:
/// ```
/// # use re_log_types::*;
/// obj_path!("foo", Index::Sequence(123));
/// ```
#[macro_export]
macro_rules! obj_path {
        () => {
            vec![]
        };
        ($($comp: expr),* $(,)?) => {
            $crate::ObjPath::from(vec![ $($crate::ObjPathComp::from($comp),)+ ])
        };
    }
