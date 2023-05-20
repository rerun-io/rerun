//! This is how we store and index logging data.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

#[cfg(feature = "serde")]
mod editable_auto_value;
pub mod entity_properties;
pub mod entity_tree;
mod instance_path;
pub mod log_db;
mod util;

pub use entity_properties::*;
pub use entity_tree::*;
pub use instance_path::*;
pub use log_db::LogDb;
pub use util::*;

#[cfg(feature = "serde")]
pub use editable_auto_value::EditableAutoValue;
use re_log_types::DataTableError;
pub use re_log_types::{ComponentName, EntityPath, EntityPathPart, Index, TimeInt, Timeline};

// ----------------------------------------------------------------------------

/// The errors that can occur when misusing the data store.
///
/// Most of these indicate a problem with either the logging SDK,
/// or how the logging SDK is being used (PEBKAC).
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error with one the underlying data table")]
    DataTable(#[from] DataTableError),

    #[error(transparent)]
    WriteError(#[from] re_arrow_store::WriteError),
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

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_scope!($($arg)*);
    };
}
