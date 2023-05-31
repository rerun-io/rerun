//! Rerun Spatial Scene Views
//!
//! Space Views that show entities in a 2D or 3D spatial relationship.

mod scene_element;
mod space_view_class;

mod eye;
mod instance_hash_conversions;
mod mesh_cache;
mod mesh_loader;
mod scene;
mod space_camera_3d;
mod transform_cache;
mod ui;

// TODO: should only make the main type public

pub mod ui_2d;
pub mod ui_3d;
pub mod ui_renderer_bridge;

pub use self::scene::{Image, MeshSource, SceneSpatial, UiLabel, UiLabelTarget};
pub use self::space_camera_3d::SpaceCamera3D;
pub use transform_cache::{TransformCache, UnreachableTransform};
pub use ui::{SpatialNavigationMode, ViewSpatialState};
pub use ui_2d::view_2d;
pub use ui_3d::{view_3d, SpaceSpecs};
