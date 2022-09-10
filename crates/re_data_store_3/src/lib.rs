mod batch;
mod full_store;
mod obj_store;
pub mod objects;
pub mod query;

pub use batch::*;
pub use full_store::*;
pub use obj_store::*;
pub use objects::{ObjectProps, Objects, ObjectsBySpace, *};

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
