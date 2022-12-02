//! This is how we store and index logging data.
//! TODO(john) better crate documentation.

pub mod field_types;
mod util;

mod arrow_log_db;
pub use self::arrow_log_db::LogDb;

mod data_store;
pub(crate) use self::data_store::{ComponentBucket, ComponentTable, IndexBucket, IndexTable};
pub use self::data_store::{ComponentName, ComponentNameRef, DataStore, RowIndex};

mod data_store_read;
pub use self::data_store_read::TimeQuery;

mod data_store_write;

pub use re_log_types::{TimeInt, TypedTimeInt}; // for politeness sake

#[cfg(test)]
mod tests;
