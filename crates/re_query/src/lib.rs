//! Provide query-centric access to the [`re_arrow_store`].
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// TODO(jleibs) better crate documentation.

mod archetype_view;
mod entity_view;
mod query;
mod range;
mod util;
pub mod visit;

#[cfg(feature = "polars")]
pub mod dataframe_util;

pub use self::archetype_view::{ArchComponentWithInstances, ArchetypeView};
pub use self::entity_view::{ComponentWithInstances, EntityView};
pub use self::query::{get_component_with_instances, query_archetype, query_entity_with_primary};
pub use self::range::range_entity_with_primary;
pub use self::util::query_primary_with_history;

// Used for doc-tests
#[doc(hidden)]
pub use self::query::__populate_example_store;

#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("Tried to access a column that doesn't exist")]
    BadAccess,

    #[error("Could not find primary")]
    PrimaryNotFound,

    #[error("Could not find component")]
    ComponentNotFound,

    #[error("Tried to access component of type '{actual:?}' using deserializer for type '{requested:?}'")]
    TypeMismatch {
        actual: re_log_types::ComponentName,
        requested: re_log_types::ComponentName,
    },

    #[error("Tried to access component of type '{actual:?}' using component '{requested:?}'")]
    NewTypeMismatch {
        actual: re_types::ComponentName,
        requested: re_types::ComponentName,
    },

    #[error("Error deserializing: {0}")]
    DeserializationError(#[from] re_types::DeserializationError),

    #[error("Error serializing: {0}")]
    SerializationError(#[from] re_types::SerializationError),

    #[error("Error with one or more the underlying data cells: {0}")]
    DataCell(#[from] re_log_types::DataCellError),

    #[error("Error converting arrow data: {0}")]
    ArrowError(#[from] arrow2::error::Error),

    #[cfg(feature = "polars")]
    #[error("Error from within Polars")]
    PolarsError(#[from] polars_core::prelude::PolarsError),
}

pub type Result<T> = std::result::Result<T, QueryError>;
