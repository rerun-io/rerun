use crate::datatypes;

use super::RotationAxisAngle;

impl RotationAxisAngle {
    /// The identity rotation, representing no rotation.
    pub const IDENTITY: Self = Self(datatypes::RotationAxisAngle::IDENTITY);

    /// Create a new rotation from an axis and an angle.
    #[inline]
    pub fn new(axis: impl Into<datatypes::Vec3D>, angle: impl Into<datatypes::Angle>) -> Self {
        Self(datatypes::RotationAxisAngle::new(axis, angle))
    }
}

#[cfg(feature = "glam")]
impl TryFrom<RotationAxisAngle> for glam::Affine3A {
    type Error = ();

    #[inline]
    fn try_from(val: RotationAxisAngle) -> Result<Self, Self::Error> {
        glam::Vec3::from(val.0.axis)
            .try_normalize()
            .map(|normalized| Self::from_axis_angle(normalized, val.0.angle.radians()))
            .ok_or(())
    }
}

#[cfg(feature = "glam")]
impl TryFrom<RotationAxisAngle> for glam::DAffine3 {
    type Error = ();

    #[inline]
    fn try_from(val: RotationAxisAngle) -> Result<Self, Self::Error> {
        glam::DVec3::from(val.0.axis)
            .try_normalize()
            .map(|normalized| Self::from_axis_angle(normalized, val.0.angle.radians() as f64))
            .ok_or(())
    }
}
