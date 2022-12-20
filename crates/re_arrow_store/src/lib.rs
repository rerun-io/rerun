//! The Rerun Arrow-based datastore.
//!
//! See `src/store.rs` for an overview of the core datastructures.

mod arrow_util;
mod store;
mod store_read;
mod store_write;

#[doc(hidden)]
pub mod test_util;

pub use self::arrow_util::ArrayExt;
pub use self::store::{DataStore, DataStoreConfig, IndexBucket, IndexTable, RowIndex};
pub use self::store_read::{TimeQuery, TimelineQuery};
pub use self::store_write::{WriteError, WriteResult};

pub(crate) use self::store::{
    ComponentBucket, ComponentTable, IndexBucketIndices, SecondaryIndex, TimeIndex,
};

// Re-exports
#[doc(no_inline)]
pub use arrow2::io::ipc::read::{StreamReader, StreamState};
#[doc(no_inline)]
pub use re_log_types::{TimeInt, TimeRange, TimeType}; // for politeness sake
