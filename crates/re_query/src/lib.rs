//! Caching datastructures for `re_query`.

// TODO(#3408): remove unwrap()
#![allow(clippy::unwrap_used)]

mod cache;
mod cache_stats;
mod flat_vec_deque;
mod latest_at;
mod promise;
mod range;

pub mod clamped_zip;
pub mod range_zip;

pub use self::cache::{CacheKey, Caches};
pub use self::cache_stats::{CachedComponentStats, CachesStats};
pub use self::clamped_zip::*;
pub use self::flat_vec_deque::{ErasedFlatVecDeque, FlatVecDeque};
pub use self::latest_at::{LatestAtComponentResults, LatestAtMonoResult, LatestAtResults};
pub use self::promise::{Promise, PromiseId, PromiseResolver, PromiseResult};
pub use self::range::{RangeComponentResults, RangeData, RangeResults};
pub use self::range_zip::*;

pub(crate) use self::latest_at::{latest_at, LatestAtCache};
pub(crate) use self::range::{RangeCache, RangeComponentResultsInner};

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

use re_data_store2::{LatestAtQuery, RangeQuery};

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

// ---

/// Returns `true` if the specified `component_name` can be cached.
///
/// Used internally to avoid unnecessarily caching components that are already cached in other
/// places, for historical reasons.
pub fn cacheable(component_name: re_types_core::ComponentName) -> bool {
    use std::sync::OnceLock;
    static NOT_CACHEABLE: OnceLock<re_types_core::ComponentNameSet> = OnceLock::new();

    // Horrible hack so we can make this work without depending on re_types.
    // We have a dev dependency on re_types only, so we test this in the tests below against the "symbolic" names.
    let component_names = [
        "rerun.components.TensorData".into(),
        "rerun.components.Blob".into(),
    ];

    let not_cacheable = NOT_CACHEABLE.get_or_init(|| component_names.into());

    !component_name.is_indicator_component() && !not_cacheable.contains(&component_name)
}

#[cfg(test)]
mod tests {
    use re_types::{
        components::{Blob, TensorData},
        Loggable as _,
    };

    use super::*;

    #[test]
    fn test_cacheable() {
        assert!(!cacheable(TensorData::name()));
        assert!(!cacheable(Blob::name()));
    }
}
