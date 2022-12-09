mod eye;
pub use self::eye::{Eye, OrbitEye};

mod mesh_cache;
pub use mesh_cache::CpuMeshCache;

mod space_camera_3d;
pub use self::space_camera_3d::SpaceCamera3D;

mod scene;
pub use self::scene::{
    Image, Label2D, Label2DTarget, Label3D, MeshSource, MeshSourceData, SceneSpatial,
};

pub mod ui_renderer_bridge;

use re_renderer::Color32;

pub const AXIS_COLOR_X: Color32 = Color32::from_rgb(255, 25, 25);
pub const AXIS_COLOR_Y: Color32 = Color32::from_rgb(0, 240, 0);
pub const AXIS_COLOR_Z: Color32 = Color32::from_rgb(80, 80, 255);

pub fn axis_color(axis: usize) -> Color32 {
    match axis {
        0 => AXIS_COLOR_X,
        1 => AXIS_COLOR_Y,
        2 => AXIS_COLOR_Z,
        _ => unreachable!("Axis should be one of 0,1,2"),
    }
}
