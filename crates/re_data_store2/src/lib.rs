//! TODO
//! The Rerun datastore, implemented on top of [Apache Arrow](https://arrow.apache.org/)
//! using the [`arrow2`] crate.
//!
//! This crate is an in-memory time series database for Rerun log data.
//! It is indexed by Entity path, component, timeline, and time.
//! It supports out-of-order insertions, and fast `O(log(N))` queries.
//!
//! * See [`DataStore`] for an overview of the core data structures.
//! * See [`DataStore::latest_at`] and [`DataStore::range`] for the documentation of the public
//!   read APIs.
//! * See [`DataStore::insert_row`] for the documentation of the public write APIs.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// TODO: we're gonna have to do some work regarding RowId uniqueness.

mod events;
mod formatting;
mod gc;
mod query;
mod stats;
mod store;
mod subscribers;
mod writes;

pub use self::events::{StoreDiff2, StoreDiffKind2, StoreEvent2};
pub use self::gc::{GarbageCollectionOptions, GarbageCollectionTarget};
pub use self::stats::{DataStoreChunkStats2, DataStoreStats2};
pub use self::store::{DataStore2, DataStoreConfig2, StoreGeneration2};
pub use self::subscribers::{StoreSubscriber2, StoreSubscriberHandle2};
pub use self::writes::{WriteError, WriteResult};

// Re-exports
#[doc(no_inline)]
pub use re_chunk::{LatestAtQuery, RangeQuery};
#[doc(no_inline)]
pub use re_log_types::{ResolvedTimeRange, TimeInt, TimeType, Timeline};

pub mod external {
    pub use re_chunk;
}
