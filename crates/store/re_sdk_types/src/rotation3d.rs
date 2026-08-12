use crate::{components, encodings};

/// A 3D rotation.
///
/// This is *not* a component, but a helper type for populating [`crate::archetypes::Transform3D`] with rotations.
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Rotation3D {
    /// Rotation defined by a quaternion.
    Quaternion(components::RotationQuat),

    /// Rotation defined with an axis and an angle.
    AxisAngle(components::RotationAxisAngle),
}

impl Rotation3D {
    /// The identity rotation, expressed as a quaternion
    pub const IDENTITY: Self = Self::Quaternion(components::RotationQuat::IDENTITY);
}

impl From<components::RotationQuat> for Rotation3D {
    #[inline]
    fn from(quat: components::RotationQuat) -> Self {
        Self::Quaternion(quat)
    }
}

impl From<encodings::Quaternion> for Rotation3D {
    #[inline]
    fn from(quat: encodings::Quaternion) -> Self {
        Self::Quaternion(quat.into())
    }
}

#[cfg(feature = "glam")]
impl From<glam::Quat> for Rotation3D {
    #[inline]
    fn from(quat: glam::Quat) -> Self {
        Self::Quaternion(quat.into())
    }
}

impl From<components::RotationAxisAngle> for Rotation3D {
    #[inline]
    fn from(axis_angle: components::RotationAxisAngle) -> Self {
        Self::AxisAngle(axis_angle)
    }
}

impl From<encodings::RotationAxisAngle> for Rotation3D {
    #[inline]
    fn from(axis_angle: encodings::RotationAxisAngle) -> Self {
        Self::AxisAngle(axis_angle.into())
    }
}
