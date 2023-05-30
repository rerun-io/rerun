use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

/// A 3D size, e.g. the size of a 3D box.
// TODO(cmc): should just embed a Vec3D?
#[derive(Debug, ArrowField, ArrowSerialize, ArrowDeserialize)]
pub struct Size3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[cfg(feature = "glam")]
impl From<Size3D> for glam::Vec3 {
    #[inline]
    fn from(size: Size3D) -> Self {
        glam::vec3(size.x, size.y, size.z)
    }
}

impl re_log_types::Component for Size3D {
    #[inline]
    fn name() -> re_log_types::ComponentName {
        "rerun.size3d".into()
    }
}
