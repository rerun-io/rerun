mod log_store;
mod objects;
mod storage;

pub use log_store::*;
pub use objects::*;
pub use storage::*;

pub use log_types::{
    Index, IndexPath, ObjPath, ObjPathBuilder, ObjPathComp, ObjTypePath, TypePathComp,
};

pub enum AtomType {
    // 1D:
    I32,
    F32,

    Color,

    // ----------------------------
    // 2D:
    Pos2,
    BBox2D,
    LineSegment2D,
    Image,

    // ----------------------------
    // 3D:
    Pos3,
    BBox3,
    Path3D,
    LineSegment3D,
    Mesh3D,
    Camera,

    // ----------------------------
    // N-D:
    Vecf32,
}

#[derive(Clone, Copy, Debug)]
pub enum Data {
    F32(f32),
    Pos3([f32; 3]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StructType {
    /// ```ignore
    /// pos: Pos3,
    /// radius: Option<F32>,
    /// color: Option<Color>,
    /// ```
    Point3D,
}

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
