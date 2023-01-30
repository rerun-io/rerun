use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use crate::msg_bundle::Component;

#[derive(Debug, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct Size3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[cfg(feature = "glam")]
impl From<Size3D> for glam::Vec3 {
    fn from(size: Size3D) -> Self {
        glam::vec3(size.x, size.y, size.z)
    }
}

impl Component for Size3D {
    fn name() -> crate::ComponentName {
        "rerun.size3d".into()
    }
}
