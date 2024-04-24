//! Caching datastructures for `re_query`.

mod cache;
mod cache_stats;
mod flat_vec_deque;
mod latest_at;
mod promise;
mod range;
mod visible_history;

pub mod clamped_zip;
pub mod range_zip;

pub use self::cache::{CacheKey, Caches};
pub use self::cache_stats::{CachedComponentStats, CachesStats};
pub use self::clamped_zip::*;
pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};
pub use self::latest_at::{
    CachedLatestAtComponentResults, CachedLatestAtMonoResult, CachedLatestAtResults,
};
pub use self::promise::{Promise, PromiseId, PromiseResolver, PromiseResult};
pub use self::range::{CachedRangeComponentResults, CachedRangeData, CachedRangeResults};
pub use self::range_zip::*;
pub use self::visible_history::{ExtraQueryHistory, VisibleHistory, VisibleHistoryBoundary};

pub(crate) use self::latest_at::LatestAtCache;
pub(crate) use self::range::{CachedRangeComponentResultsInner, RangeCache};

pub mod external {
    pub use paste;
    pub use seq_macro;
}

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
pub enum CachedResults {
    LatestAt(LatestAtQuery, CachedLatestAtResults),
    Range(RangeQuery, CachedRangeResults),
}

impl From<(LatestAtQuery, CachedLatestAtResults)> for CachedResults {
    #[inline]
    fn from((query, results): (LatestAtQuery, CachedLatestAtResults)) -> Self {
        Self::LatestAt(query, results)
    }
}

impl From<(RangeQuery, CachedRangeResults)> for CachedResults {
    #[inline]
    fn from((query, results): (RangeQuery, CachedRangeResults)) -> Self {
        Self::Range(query, results)
    }
}
