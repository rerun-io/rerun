//! This is how we store and index logging data.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#[cfg(feature = "serde")]
mod blueprint;
#[cfg(feature = "serde")]
mod editable_auto_value;
pub mod entity_properties;
pub mod entity_tree;
mod instance_path;
pub mod store_db;
mod versioned_instance_path;

pub use self::entity_properties::*;
pub use self::entity_tree::*;
pub use self::instance_path::{InstancePath, InstancePathHash};
pub use self::store_db::StoreDb;
pub use self::versioned_instance_path::{VersionedInstancePath, VersionedInstancePathHash};

#[cfg(feature = "serde")]
pub use blueprint::EntityPropertiesComponent;
#[cfg(feature = "serde")]
pub use editable_auto_value::EditableAutoValue;
use re_log_types::DataTableError;
pub use re_log_types::{EntityPath, EntityPathPart, Index, TimeInt, Timeline};

// ----------------------------------------------------------------------------

/// The errors that can occur when misusing the data store.
///
/// Most of these indicate a problem with either the logging SDK,
/// or how the logging SDK is being used (PEBKAC).
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("The incoming data was inconsistent: {0}")]
    DataRead(#[from] re_log_types::DataReadError),

    #[error("Error with one the underlying data table: {0}")]
    DataTable(#[from] DataTableError),

    #[error(transparent)]
    Write(#[from] re_arrow_store::WriteError),

    #[error(transparent)]
    DataRow(#[from] re_log_types::DataRowError),
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
