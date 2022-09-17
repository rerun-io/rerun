//! This is how we store and index logging data.
//!
//! We partion the data in this order:
//! * [`TimeSource`]
//! * [`ObjPath`]
//! * [`FieldName`]
//! * [`TimeInt`]
//!
//! The stores are in order:
//! * [`DataStore`], which maps [`TimeSource`] to…
//! * [`TimeLineStore`], which maps [`ObjPath`] to…
//! * [`ObjStore`], which maps [`FieldName`] to…
//! * [`FieldStore`], which maps [`TimeInt`] to values.
//!
//! (in fact, most stores are generic on what the time type is, but in practice it is [`TimeInt`]).

mod batch;
mod instance;
pub mod log_db;
pub mod objects;
pub mod query;
mod stores;

pub use batch::*;
pub use instance::*;
pub use objects::*;
pub use stores::*;

use re_log_types::DataType;

pub use re_log_types::{
    FieldName, Index, IndexPath, ObjPath, ObjPathComp, ObjTypePath, TimeInt, TimeSource,
    TypePathComp,
};

// ----------------------------------------------------------------------------

/// The errors that can occur when misuing the data store.
///
/// Most of these indicate a problem with either the logging SDK,
/// or how the loggign SDK is being used (PEBKAC).
#[derive(thiserror::Error, Clone, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("Batch had differing number of indices and data.")]
    BadBatch,

    #[error("Using an object both as mono and multi.")]
    MixingMonoAndMulti,

    #[error(
        "Logging different types to the same field. Existing: {existing:?}, expected: {expected:?}"
    )]
    MixingTypes {
        existing: DataType,
        expected: DataType,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

// ----------------------------------------------------------------------------

/// A query in time.
pub enum TimeQuery<Time> {
    /// Get the latest version of the data available at this time.
    LatestAt(Time),

    /// Get all the data within this time interval, plus the latest
    /// one before the start of the interval.
    ///
    /// Motivation: all data is considered alive untl the next logging
    /// to the same data path.
    Range(std::ops::RangeInclusive<Time>),
}

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(all(feature = "puffin", not(target_arch = "wasm32")))]
        puffin::profile_scope!($($arg)*);
    };
}
