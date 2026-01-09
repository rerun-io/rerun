//! This is how we store and index logging data.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod chunk_promise;
pub mod entity_db;
pub mod entity_tree;
mod ingestion_statistics;
mod instance_path;
mod rrd_manifest_index;
mod sorted_range_map;
mod store_bundle;
mod time_histogram_per_timeline;
mod versioned_instance_path;

#[doc(no_inline)]
pub use re_log_types::{EntityPath, EntityPathPart, TimeInt, Timeline};

pub use self::entity_db::{DEFAULT_GC_TIME_BUDGET, EntityDb};
pub use self::entity_tree::EntityTree;
pub use self::ingestion_statistics::{IngestionStatistics, LatencySnapshot, LatencyStats};
pub use self::instance_path::{InstancePath, InstancePathHash};
pub use self::rrd_manifest_index::{ChunkPrefetchOptions, LoadState, RrdManifestIndex};
pub use self::store_bundle::{StoreBundle, StoreLoadError};
pub use self::time_histogram_per_timeline::{TimeHistogram, TimeHistogramPerTimeline};
pub use self::versioned_instance_path::{VersionedInstancePath, VersionedInstancePathHash};

pub mod external {
    pub use {re_chunk_store, re_query};
}

// ----------------------------------------------------------------------------

/// The errors that can occur when misusing the chunk store.
///
/// Most of these indicate a problem with either the logging SDK,
/// or how the logging SDK is being used (PEBKAC).
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Write(#[from] re_chunk_store::ChunkStoreError),

    #[error(transparent)]
    Chunk(#[from] re_chunk::ChunkError),
}

pub type Result<T> = std::result::Result<T, Error>;

// ----------------------------------------------------------------------------

/// A query in time.
#[derive(Clone, Debug)]
pub enum TimeQuery<Time> {
    /// Get the latest version of the data available at this time.
    LatestAt(Time),

    /// Get all the data within this time interval, plus the latest
    /// one before the start of the interval.
    ///
    /// Motivation: all data is considered alive until the next logging
    /// to the same component path.
    Range(std::ops::RangeInclusive<Time>),
}

impl TimeQuery<i64> {
    pub const EVERYTHING: Self = Self::Range(i64::MIN..=i64::MAX);
}
