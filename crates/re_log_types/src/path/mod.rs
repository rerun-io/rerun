//! Every logged entity in Rerun is logged to an [`EntityPath`].
//!
//! The path is made up out of several [`EntityPathPart`]s,
//! each of which is either a name ([`EntityPathPart::Name`])
//! or an [`Index`].
//!
//! The [`Index`]es are for tables, arrays etc.

mod component_path;
mod entity_path;
mod entity_path_impl;
mod parse_path;

pub use component_path::ComponentPath;
pub use entity_path::{EntityPath, EntityPathHash};
pub use entity_path_impl::EntityPathImpl;
pub use parse_path::PathParseError;

use re_string_interner::InternedString;

use crate::Index;

// ----------------------------------------------------------------------------

/// The different parts that make up an [`EntityPath`].
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum EntityPathPart {
    /// Struct member. Each member can have a different type.
    Name(InternedString),

    /// Array/table/map member. Each member must be of the same type (homogeneous).
    Index(Index),
}

impl std::fmt::Display for EntityPathPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(name) => f.write_str(name),
            Self::Index(index) => index.fmt(f),
        }
    }
}

impl From<&str> for EntityPathPart {
    #[inline]
    fn from(part: &str) -> Self {
        Self::Name(part.into())
    }
}

impl From<String> for EntityPathPart {
    #[inline]
    fn from(part: String) -> Self {
        Self::Name(part.into())
    }
}

impl From<Index> for EntityPathPart {
    #[inline]
    fn from(part: Index) -> Self {
        Self::Index(part)
    }
}

// ----------------------------------------------------------------------------

/// Build a `Vec<EntityPathPart>`:
/// ```
/// # use re_log_types::*;
/// entity_path_vec!("foo", Index::Sequence(123));
/// ```
#[macro_export]
macro_rules! entity_path_vec {
    () => {
        vec![]
    };
    ($($part: expr),* $(,)?) => {
        vec![ $($crate::EntityPathPart::from($part),)+ ]
    };
}

/// Build a `EntityPath`:
/// ```
/// # use re_log_types::*;
/// entity_path!("foo", Index::Sequence(123));
/// ```
#[macro_export]
macro_rules! entity_path {
    () => {
        vec![]
    };
    ($($part: expr),* $(,)?) => {
        $crate::EntityPath::from(vec![ $($crate::EntityPathPart::from($part),)+ ])
    };
}
