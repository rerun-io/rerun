mod log_store;
mod objects;
mod storage;

pub use log_store::*;
pub use objects::*;
pub use storage::*;

pub use log_types::{
    Index, IndexPath, ObjPath, ObjPathBuilder, ObjPathComp, ObjTypePath, TypePathComp,
};

// ----------------------------------------------------------------------------

/// Path to the object owning the batch, i.e. stopping before the last index
///
/// `index_path_prefix` should have `Index::Placeholder` in the last position.
pub(crate) fn batch_parent_obj_path(
    type_path: &ObjTypePath,
    index_path_prefix: &IndexPath,
) -> ObjPath {
    let mut index_it = index_path_prefix.iter();

    let mut obj_type_path = vec![];
    let mut index_path = vec![];

    let mut found_placeholder = false;

    for typ in type_path {
        match typ {
            TypePathComp::String(name) => {
                obj_type_path.push(TypePathComp::String(*name));
            }
            TypePathComp::Index => {
                if let Some(index) = index_it.next() {
                    if matches!(index, Index::Placeholder) {
                        assert!(
                            !found_placeholder,
                            "Not a batch path. type_path: {}, index_path_prefix: {:?}",
                            type_path, index_path_prefix
                        );
                        found_placeholder = true;
                    } else {
                        obj_type_path.push(TypePathComp::Index);
                        index_path.push(index.clone());
                    }
                } else {
                    panic!(
                        "Not a batch path. type_path: {}, index_path_prefix: {:?}",
                        type_path, index_path_prefix
                    );
                }
            }
        }
    }

    assert!(
        found_placeholder,
        "Not a batch path. type_path: {}, index_path_prefix: {:?}",
        type_path, index_path_prefix
    );

    ObjPath::new(ObjTypePath::new(obj_type_path), IndexPath::new(index_path))
}

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
