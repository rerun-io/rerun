//! Caching datastructures for `re_query`.

mod cache;
mod cache_stats;
mod latest_at;
mod range;
mod storage_engine;

pub mod clamped_zip;
pub mod range_zip;

use re_chunk::ComponentIdentifier;

pub use self::{
    cache::{QueryCache, QueryCacheHandle, QueryCacheKey},
    cache_stats::{QueryCacheStats, QueryCachesStats},
    clamped_zip::*,
    latest_at::LatestAtResults,
    range::RangeResults,
    range_zip::*,
    storage_engine::{
        StorageEngine, StorageEngineArcReadGuard, StorageEngineLike, StorageEngineReadGuard,
        StorageEngineWriteGuard,
    },
};
pub(crate) use self::{latest_at::LatestAtCache, range::RangeCache};

pub mod external {
    pub use paste;
    pub use seq_macro;
}

// ---

#[derive(Debug, Clone, Copy)]
pub struct ComponentNotFoundError(pub re_types_core::ComponentType);

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
    PrimaryNotFound(ComponentIdentifier),

    #[error(transparent)]
    ComponentNotFound(#[from] ComponentNotFoundError),

    #[error("Tried to access component of type '{actual:?}' using component '{requested:?}'")]
    TypeMismatch {
        actual: re_types_core::ComponentType,
        requested: re_types_core::ComponentType,
    },

    #[error("Error deserializing: {0}")]
    DeserializationError(#[from] re_types_core::DeserializationError),

    #[error("Error serializing: {0}")]
    SerializationError(#[from] re_types_core::SerializationError),

    #[error("Not implemented")]
    NotImplemented,

    #[error("{}", re_error::format(.0))]
    Other(#[from] anyhow::Error),
}

const _: () = assert!(
    std::mem::size_of::<QueryError>() <= 80,
    "Error type is too large. Try to reduce its size by boxing some of its variants.",
);

pub type Result<T> = std::result::Result<T, QueryError>;
