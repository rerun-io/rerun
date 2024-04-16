//! Provide query-centric access to the [`re_data_store`].

mod latest_at;
mod promise;
mod range;
mod visible_history;

pub mod clamped_zip;
pub mod range_zip;

pub use self::clamped_zip::*;
pub use self::latest_at::{latest_at, LatestAtComponentResults, LatestAtResults};
pub use self::promise::{Promise, PromiseId, PromiseResolver, PromiseResult};
pub use self::range::{range, RangeComponentResults, RangeResults};
pub use self::range_zip::*;
pub use self::visible_history::{ExtraQueryHistory, VisibleHistory, VisibleHistoryBoundary};

// ---

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

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, QueryError>;

// ---

/// Helper extension trait to convert query results into [`re_types_core::Archetype`]s.
pub trait ToArchetype<A: re_types_core::Archetype> {
    /// Converts the result into an [`re_types_core::Archetype`].
    ///
    /// Automatically handles all aspects of the query process: deserialization, caching, promise
    /// resolution, etc.
    fn to_archetype(
        &self,
        resolver: &crate::PromiseResolver,
    ) -> crate::PromiseResult<crate::Result<A>>;
}

// ---

use re_data_store::{LatestAtQuery, RangeQuery};

#[derive(Debug)]
pub enum Results {
    LatestAt(LatestAtQuery, LatestAtResults),
    Range(RangeQuery, RangeResults),
}

impl From<(LatestAtQuery, LatestAtResults)> for Results {
    #[inline]
    fn from((query, results): (LatestAtQuery, LatestAtResults)) -> Self {
        Self::LatestAt(query, results)
    }
}

impl From<(RangeQuery, RangeResults)> for Results {
    #[inline]
    fn from((query, results): (RangeQuery, RangeResults)) -> Self {
        Self::Range(query, results)
    }
}
