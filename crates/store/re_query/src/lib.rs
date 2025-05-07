//! Caching datastructures for `re_query`.

mod cache;
mod cache_stats;
mod chunks_with_descriptor;
mod latest_at;
mod range;
mod storage_engine;

pub mod clamped_zip;
pub mod range_zip;

pub use self::cache::{QueryCache, QueryCacheHandle, QueryCacheKey};
pub use self::cache_stats::{CacheStats, CachesStats};
pub use self::chunks_with_descriptor::{ChunkWithDescriptor, ChunksWithDescriptor};
pub use self::clamped_zip::*;
pub use self::latest_at::LatestAtResults;
pub use self::range::RangeResults;
pub use self::range_zip::*;
pub use self::storage_engine::{
    StorageEngine, StorageEngineArcReadGuard, StorageEngineLike, StorageEngineReadGuard,
    StorageEngineWriteGuard,
};

pub(crate) use self::latest_at::LatestAtCache;
pub(crate) use self::range::RangeCache;

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
    PrimaryNotFound(re_types_core::ComponentDescriptor),

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

    #[error("Not implemented")]
    NotImplemented,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, QueryError>;

// ---

use re_chunk::ComponentName;
use re_types_core::ComponentDescriptor;

// TODO(#6889): This is a temporary object until we use tagged components everywhere or have explicit untagged queries (?).
pub enum MaybeTagged {
    Descriptor(ComponentDescriptor),
    JustName(ComponentName),
}

impl<'a> From<&'a ComponentDescriptor> for MaybeTagged {
    #[inline]
    fn from(descr: &'a ComponentDescriptor) -> Self {
        Self::Descriptor(descr.clone())
    }
}

impl From<ComponentDescriptor> for MaybeTagged {
    #[inline]
    fn from(descr: ComponentDescriptor) -> Self {
        Self::Descriptor(descr)
    }
}

impl From<ComponentName> for MaybeTagged {
    #[inline]
    fn from(name: ComponentName) -> Self {
        Self::JustName(name)
    }
}
