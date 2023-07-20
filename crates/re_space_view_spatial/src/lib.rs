//! Rerun Spatial Space Views
//!
//! Space Views that show entities in a 2D or 3D spatial relationship.

mod contexts;
mod eye;
mod heuristics;
mod instance_hash_conversions;
mod mesh_cache;
mod mesh_loader;
mod parts;
mod picking;
mod space_camera_3d;
mod space_view_2d;
mod space_view_3d;
mod ui;
mod ui_2d;
mod ui_3d;

pub use space_view_2d::SpatialSpaceView2D;
pub use space_view_3d::SpatialSpaceView3D;

// ---

mod view_kind {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SpatialSpaceViewKind {
        TwoD,
        ThreeD,
    }
}
