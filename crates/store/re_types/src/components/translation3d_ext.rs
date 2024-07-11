#[cfg(feature = "glam")]
use super::Translation3D;

// This is intentionally not implemented for `Vec3`:
// The transform semantic is expressed here, `Vec3` on the other hand implements conversion to `glam::Vec3A`.
#[cfg(feature = "glam")]
impl From<Translation3D> for glam::Affine3A {
    #[inline]
    fn from(v: Translation3D) -> Self {
        Self {
            matrix3: glam::Mat3A::IDENTITY,
            translation: glam::Vec3A::from_slice(&v.0 .0),
        }
    }
}
