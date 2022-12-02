//! This is how we store and index logging data.
//! TODO(john) better crate documentation.

mod field_types;
mod read;
mod store;
#[cfg(test)]
pub mod tests;
mod write;

pub use self::read::TimeQuery;
pub(crate) use self::store::{ComponentBucket, ComponentTable, IndexBucket, IndexTable};
pub use self::store::{ComponentName, ComponentNameRef, DataStore, RowIndex};
pub use re_log_types::{TimeInt, TypedTimeInt}; // for politeness sake

// Re-export
pub use arrow2::io::ipc::read::{StreamReader, StreamState};

/// Build a [`StreamReader`] from a slice of `u8`
pub fn build_stream_reader(data: &[u8]) -> StreamReader<impl std::io::Read + '_> {
    let mut cursor = std::io::Cursor::new(data);
    let metadata = arrow2::io::ipc::read::read_stream_metadata(&mut cursor).unwrap();
    StreamReader::new(cursor, metadata, None)
}
