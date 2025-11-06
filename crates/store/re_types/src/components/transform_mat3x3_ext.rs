#[cfg(feature = "glam")]
use super::TransformMat3x3;

// This is intentionally not implemented for `Mat3x3`:
// The transform semantic is expressed here, `Mat3x3` on the other hand implements conversion to `glam::Mat3A`.
#[cfg(feature = "glam")]
impl From<TransformMat3x3> for glam::Affine3A {
    #[inline]
    fn from(v: TransformMat3x3) -> Self {
        Self {
            matrix3: glam::Mat3A::from_cols_slice(&v.0.0),
            translation: glam::Vec3A::ZERO,
        }
    }
}
