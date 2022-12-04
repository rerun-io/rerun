//! This is how we store and index logging data.
//! TODO(john) better crate documentation.

mod field_types;
mod store;
mod store_read;
mod store_write;

#[cfg(feature = "datagen")]
pub mod datagen;

pub(crate) use self::store::{ComponentBucket, ComponentTable, IndexBucket, IndexTable};
pub use self::store::{ComponentName, ComponentNameRef, DataStore, RowIndex};
pub use self::store_read::TimeQuery;

// Re-exports
pub use arrow2::io::ipc::read::{StreamReader, StreamState};
pub use re_log_types::{TimeInt, TimeType, TypedTimeInt}; // for politeness sake

/// Build a [`StreamReader`] from a slice of `u8`
pub fn build_stream_reader(data: &[u8]) -> StreamReader<impl std::io::Read + '_> {
    let mut cursor = std::io::Cursor::new(data);
    let metadata = arrow2::io::ipc::read::read_stream_metadata(&mut cursor).unwrap();
    StreamReader::new(cursor, metadata, None)
}
