//! The Rerun Arrow-based datastore.
//!
//! * See [`DataStore`] for an overview of the core datastructures.
//! * See [`DataStore::latest_at`] and [`DataStore::range`] for the documentation of the public
//!   read APIs.
//! * See [`DataStore::insert`] for the documentation of the public write APIs.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

mod arrow_util;
mod store;
mod store_gc;
mod store_read;
mod store_stats;
mod store_write;

#[cfg(feature = "polars")]
mod store_polars;

#[cfg(feature = "polars")]
pub mod polars_util;

#[doc(hidden)]
pub mod test_util;

pub use self::arrow_util::ArrayExt;
pub use self::store::{
    DataStore, DataStoreConfig, IndexBucket, IndexRowNr, IndexTable, RowIndex, RowIndexKind,
};
pub use self::store_gc::GarbageCollectionTarget;
pub use self::store_read::{LatestAtQuery, RangeQuery};
pub use self::store_stats::DataStoreStats;
pub use self::store_write::{WriteError, WriteResult};

pub(crate) use self::store::{
    ComponentBucket, ComponentTable, IndexBucketIndices, PersistentComponentTable,
    PersistentIndexTable, SecondaryIndex, TimeIndex,
};

// Re-exports
#[doc(no_inline)]
pub use arrow2::io::ipc::read::{StreamReader, StreamState};
#[doc(no_inline)]
pub use re_log_types::{TimeInt, TimeRange, TimeType, Timeline}; // for politeness sake

// ---

/// Native-only profiling macro for puffin.
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_function!($($arg)*);
    };
}

/// Native-only profiling macro for puffin.
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_scope!($($arg)*);
    };
}
