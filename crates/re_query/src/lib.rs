//! Provide query-centric access to the [`re_data_store`].
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// TODO(jleibs) better crate documentation.

mod archetype_view;
mod query;
mod range;
mod util;

pub use self::archetype_view::{ArchetypeView, ComponentWithInstances};
pub use self::query::{get_component_with_instances, query_archetype};
pub use self::range::range_archetype;
pub use self::util::query_archetype_with_history;

// Used for doc-tests
#[cfg(feature = "testing")]
pub use self::query::__populate_example_store;

#[derive(Debug, Clone, Copy)]
pub struct ComponentNotFoundError(pub re_types_core::ComponentName);

impl std::fmt::Display for ComponentNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Could not find component: {}", self.0))
    }
}

impl std::error::Error for ComponentNotFoundError {}

#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("Tried to access a column that doesn't exist")]
    BadAccess,

    #[error("Could not find primary component: {0}")]
    PrimaryNotFound(re_types_core::ComponentName),

    #[error(transparent)]
    ComponentNotFound(#[from] ComponentNotFoundError),

    #[error("Tried to access component of type '{actual:?}' using component '{requested:?}'")]
    TypeMismatch {
        actual: re_types_core::ComponentName,
        requested: re_types_core::ComponentName,
    },

    #[error("Error with one or more the underlying data cells: {0}")]
    DataCell(#[from] re_log_types::DataCellError),

    #[error("Error deserializing: {0}")]
    DeserializationError(#[from] re_types_core::DeserializationError),

    #[error("Error serializing: {0}")]
    SerializationError(#[from] re_types_core::SerializationError),

    #[error("Error converting arrow data: {0}")]
    ArrowError(#[from] arrow2::error::Error),

    #[error("Not implemented")]
    NotImplemented,
}

pub type Result<T> = std::result::Result<T, QueryError>;
