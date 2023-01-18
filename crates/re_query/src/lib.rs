//! Provide query-centric access to the `re_arrow_store`
//! TODO(jleibs) better crate documentation.

mod entity_view;
mod query;
mod range;
mod util;
pub mod visit;

#[cfg(feature = "polars")]
pub mod dataframe_util;

pub use self::entity_view::{ComponentWithInstances, EntityView};
pub use self::query::{get_component_with_instances, query_entity_with_primary};
pub use self::range::range_entity_with_primary;
pub use self::util::ugly_query_helper;

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

    #[error("Error converting arrow data")]
    ArrowError(#[from] arrow2::error::Error),

    #[cfg(feature = "polars")]
    #[error("Error from within Polars")]
    PolarsError(#[from] polars_core::prelude::PolarsError),
}

pub type Result<T> = std::result::Result<T, QueryError>;
