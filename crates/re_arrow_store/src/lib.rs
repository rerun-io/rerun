//! This is how we store and index logging data.
//! TODO(john) better crate documentation.

//! The Rerun Arrow-based datastore.
//!
//! See `src/store.rs` for an overview of the core datastructures.
//!

mod store;
mod store_read;
mod store_write;

pub(crate) use self::store::{
    ComponentBucket, ComponentTable, IndexBucketIndices, SecondaryIndex, TimeIndex,
};
pub use self::store::{DataStore, DataStoreConfig, IndexBucket, IndexTable, RowIndex};
pub use self::store_read::{TimeQuery, TimelineQuery};

// Re-exports
#[doc(no_inline)]
pub use arrow2::io::ipc::read::{StreamReader, StreamState};
#[doc(no_inline)]
pub use re_log_types::{TimeInt, TimeRange, TimeType}; // for politeness sake

/// Build a [`StreamReader`] from a slice of `u8`
pub fn build_stream_reader(data: &[u8]) -> StreamReader<impl std::io::Read + '_> {
    let mut cursor = std::io::Cursor::new(data);
    let metadata = arrow2::io::ipc::read::read_stream_metadata(&mut cursor).unwrap();
    StreamReader::new(cursor, metadata, None)
}
