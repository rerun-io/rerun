//! This is how we store and index logging data.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

pub mod entity_db;
pub mod entity_tree;
mod instance_path;
mod store_bundle;
mod time_histogram_per_timeline;
mod times_per_timeline;
mod versioned_instance_path;

pub use self::{
    entity_db::{DEFAULT_GC_TIME_BUDGET, EntityDb},
    entity_tree::EntityTree,
    instance_path::{InstancePath, InstancePathHash},
    store_bundle::{
        DatasetRecordings, LocalRecordings, RemoteRecordings, SortDatasetsResults, StoreBundle,
        StoreLoadError,
    },
    time_histogram_per_timeline::{TimeHistogram, TimeHistogramPerTimeline},
    times_per_timeline::{TimeCounts, TimesPerTimeline},
    versioned_instance_path::{VersionedInstancePath, VersionedInstancePathHash},
};

#[doc(no_inline)]
pub use re_log_types::{EntityPath, EntityPathPart, TimeInt, Timeline};

pub mod external {
    pub use re_chunk_store;
    pub use re_query;
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
