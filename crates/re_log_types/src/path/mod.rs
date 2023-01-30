//! Every logged object in Rerun is logged to a [`EntityPath`].
//!
//! The path is made up out of several [`EntityPathComponent`],
//! each of which is either a name ([`EntityPathComponent::Name`])
//! or an [`Index`].
//!
//! The [`Index`]es are for tables, arrays etc.

mod data_path;
mod entity_path;
mod entity_path_impl;
mod parse_path;

pub use data_path::DataPath;
pub use entity_path::{EntityPath, EntityPathHash};
pub use entity_path_impl::EntityPathImpl;
pub use parse_path::{parse_entity_path, PathParseError};

use re_string_interner::InternedString;

use crate::Index;

re_string_interner::declare_new_type!(
    /// The name of an object component, e.g. `pos` or `color`.
    pub struct ComponentName;
);

// ----------------------------------------------------------------------------

/// The different parts that make up an [`EntityPath`].
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum EntityPathComponent {
    /// Struct member. Each member can have a different type.
    Name(InternedString),

    /// Array/table/map member. Each member must be of the same type (homogenous).
    Index(Index),
}

impl std::fmt::Display for EntityPathComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(name) => f.write_str(name),
            Self::Index(index) => index.fmt(f),
        }
    }
}

impl From<&str> for EntityPathComponent {
    #[inline]
    fn from(comp: &str) -> Self {
        Self::Name(comp.into())
    }
}

impl From<String> for EntityPathComponent {
    #[inline]
    fn from(comp: String) -> Self {
        Self::Name(comp.into())
    }
}

impl From<Index> for EntityPathComponent {
    #[inline]
    fn from(comp: Index) -> Self {
        Self::Index(comp)
    }
}

// ----------------------------------------------------------------------------

/// Build a `Vec<EntityPathComponent>`:
/// ```
/// # use re_log_types::*;
/// entity_path_vec!("foo", Index::Sequence(123));
/// ```
#[macro_export]
macro_rules! entity_path_vec {
        () => {
            vec![]
        };
        ($($comp: expr),* $(,)?) => {
            vec![ $($crate::EntityPathComponent::from($comp),)+ ]
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
        ($($comp: expr),* $(,)?) => {
            $crate::EntityPath::from(vec![ $($crate::EntityPathComponent::from($comp),)+ ])
        };
    }
