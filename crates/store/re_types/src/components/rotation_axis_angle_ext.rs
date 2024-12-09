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
impl From<RotationAxisAngle> for glam::Affine3A {
    #[inline]
    fn from(val: RotationAxisAngle) -> Self {
        if let Some(normalized) = glam::Vec3::from(val.0.axis).try_normalize() {
            Self::from_axis_angle(normalized, val.0.angle.radians())
        } else {
            // If the axis is zero length, we can't normalize it, so we just use the identity rotation.
            Self::IDENTITY
        }
    }
}
