mod batch;
mod data_store;
mod field_store;
mod instance;
mod obj_store;
pub mod objects;
pub mod query;
mod timeline_store;

pub use batch::*;
pub use data_store::*;
pub use field_store::*;
pub use instance::*;
pub use obj_store::*;
pub use objects::{ObjectProps, Objects, ObjectsBySpace, *};
pub use timeline_store::*;

use re_log_types::DataType;

pub use re_log_types::{Index, IndexPath, ObjPath, ObjPathComp, ObjTypePath, TypePathComp};

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Using an object both as mono and multi.
    MixingMonoAndMulti,

    /// Logging different types to the same field.
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

    /// Get all the data within this time interval.
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
