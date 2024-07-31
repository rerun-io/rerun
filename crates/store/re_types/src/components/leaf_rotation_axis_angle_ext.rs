use crate::datatypes;

use super::LeafRotationAxisAngle;

impl LeafRotationAxisAngle {
    /// Create a new rotation from an axis and an angle.
    #[inline]
    pub fn new(axis: impl Into<datatypes::Vec3D>, angle: impl Into<datatypes::Angle>) -> Self {
        Self(datatypes::RotationAxisAngle::new(axis, angle))
    }
}

#[cfg(feature = "glam")]
impl From<LeafRotationAxisAngle> for glam::Affine3A {
    #[inline]
    fn from(val: LeafRotationAxisAngle) -> Self {
        Self::from_axis_angle(val.0.axis.into(), val.0.angle.radians())
    }
}
