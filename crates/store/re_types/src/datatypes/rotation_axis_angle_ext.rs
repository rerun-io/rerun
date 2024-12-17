use super::RotationAxisAngle;

use crate::datatypes::{Angle, Vec3D};

impl RotationAxisAngle {
    /// The identity rotation, representing no rotation.
    pub const IDENTITY: Self = Self {
        axis: Vec3D([1.0, 0.0, 0.0]), // Might as well use a zero vector here, but putting in the X axis is less error prone.
        angle: Angle::ZERO,
    };

    /// A rotation that represents an invalid transform.
    pub const INVALID: Self = Self {
        axis: Vec3D::ZERO,
        angle: Angle::ZERO,
    };

    /// Create a new rotation from an axis and an angle.
    #[inline]
    pub fn new(axis: impl Into<Vec3D>, angle: impl Into<Angle>) -> Self {
        Self {
            axis: axis.into(),
            angle: angle.into(),
        }
    }
}

impl<V: Into<Vec3D>, A: Into<Angle>> From<(V, A)> for RotationAxisAngle {
    fn from((axis, angle): (V, A)) -> Self {
        Self::new(axis, angle)
    }
}

#[cfg(feature = "glam")]
impl TryFrom<RotationAxisAngle> for glam::Quat {
    type Error = ();

    #[inline]
    fn try_from(val: RotationAxisAngle) -> Result<Self, ()> {
        let axis: glam::Vec3 = val.axis.into();
        axis.try_normalize()
            .map(|axis| Self::from_axis_angle(axis, val.angle.radians()))
            .ok_or(())
    }
}

#[cfg(feature = "mint")]
impl From<RotationAxisAngle> for mint::Quaternion<f32> {
    #[inline]
    fn from(val: RotationAxisAngle) -> Self {
        let (s, c) = (val.angle.radians() * 0.5).sin_cos();
        [val.axis.x() * s, val.axis.y() * s, val.axis.z() * s, c].into()
    }
}

impl Default for RotationAxisAngle {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}
