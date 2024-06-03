//! The Rerun datastore, implemented on top of [Apache Arrow](https://arrow.apache.org/)
//! using the [`arrow2`] crate.
//!
//! This crate is an in-memory time series database for Rerun log data.
//! It is indexed by entity path, component, timeline, and time.
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

mod arrow_util;
mod store;
mod store_arrow;
mod store_dump;
mod store_event;
mod store_format;
mod store_gc;
mod store_helpers;
mod store_read;
mod store_sanity;
mod store_stats;
mod store_subscriber;
mod store_write;

#[doc(hidden)]
pub mod test_util;

pub use self::store::{DataStore, DataStoreConfig, StoreGeneration};
pub use self::store_event::{StoreDiff, StoreDiffKind, StoreEvent};
pub use self::store_gc::{GarbageCollectionOptions, GarbageCollectionTarget};
pub use self::store_read::{LatestAtQuery, RangeQuery};
pub use self::store_stats::{DataStoreRowStats, DataStoreStats, EntityStats};
pub use self::store_subscriber::{StoreSubscriber, StoreSubscriberHandle};
pub use self::store_write::{WriteError, WriteResult};

pub(crate) use self::store::{
    IndexedBucket, IndexedBucketInner, IndexedTable, MetadataRegistry, StaticCell, StaticTable,
};

// Re-exports
#[doc(no_inline)]
pub use arrow2::io::ipc::read::{StreamReader, StreamState};
#[doc(no_inline)]
pub use re_log_types::{ResolvedTimeRange, TimeInt, TimeType, Timeline}; // for politeness sake

pub mod external {
    pub use arrow2;
}

// ---

/// The index of a row's worth of data.
///
/// Every row of data in Rerun is uniquely identified, and globally ordered, by
/// its index: a _data time_ and a row ID.
///
/// ## Ordering
///
/// The ordering of two `DataIndex`es is di
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataIndex(pub TimeInt, pub re_log_types::RowId);

impl PartialOrd for DataIndex {
    #[inline]
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for DataIndex {
    #[inline]
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        match (*self, *rhs) {
            (Self(TimeInt::STATIC, lhs_row_id), Self(TimeInt::STATIC, rhs_row_id)) => {
                lhs_row_id.cmp(&rhs_row_id)
            }
            (_, Self(TimeInt::STATIC, _)) => std::cmp::Ordering::Less,
            (Self(TimeInt::STATIC, _), _) => std::cmp::Ordering::Greater,
            (Self(lhs_data_time, lhs_row_id), Self(rhs_data_time, rhs_row_id)) => {
                (lhs_data_time, lhs_row_id).cmp(&(rhs_data_time, rhs_row_id))
            }
        }
    }
}

impl DataIndex {
    #[inline]
    pub fn data_time(&self) -> TimeInt {
        self.0
    }

    #[inline]
    pub fn row_id(&self) -> re_log_types::RowId {
        self.1
    }
}

// TODO
#[test]
fn data_index_ordering() {}
