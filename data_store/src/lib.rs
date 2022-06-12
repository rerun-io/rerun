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

/// Marker traits for types we allow in the the data store.
///
/// Everything in [`data_types`] implement this, and nothing else.
pub trait DataType: 'static + Clone {}

pub mod data_types {
    use super::DataType;

    impl DataType for i32 {}
    impl DataType for f32 {}

    pub type Vec2 = [f32; 2];
    impl DataType for Vec2 {}

    pub type LineSegment2D = [Vec2; 2];
    impl DataType for LineSegment2D {}

    pub type LineSegment3D = [Vec3; 2];
    impl DataType for LineSegment3D {}

    pub type Vec3 = [f32; 3];
    impl DataType for Vec3 {}

    pub type Color = [u8; 4];
    impl DataType for Color {}

    impl DataType for log_types::BBox2D {}
    impl DataType for log_types::Box3 {}
    impl DataType for log_types::Camera {}
    impl DataType for log_types::Image {}
    impl DataType for log_types::Mesh3D {}
    impl DataType for log_types::ObjPath {}

    /// For batches
    impl<T: DataType> DataType for Vec<T> {}
}

// ----------------------------------------------------------------------------

/// Path to the object owning the batch, i.e. stopping before the last index
pub(crate) fn batch_parent_obj_path(
    type_path: &ObjTypePath,
    index_path_prefix: &IndexPath,
) -> ObjPath {
    let mut index_it = index_path_prefix.iter();

    let mut obj_type_path = vec![];
    let mut index_path = vec![];

    for typ in type_path {
        match typ {
            TypePathComp::String(name) => {
                obj_type_path.push(TypePathComp::String(*name));
            }
            TypePathComp::Index => {
                if let Some(index) = index_it.next() {
                    obj_type_path.push(TypePathComp::Index);
                    index_path.push(index.clone());
                } else {
                    return ObjPath::new(
                        ObjTypePath::new(obj_type_path),
                        IndexPath::new(index_path),
                    );
                }
            }
        }
    }

    panic!("Not a batch path");
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
