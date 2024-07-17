use crate::{
    components::{RotationAxisAngle, RotationQuat, Scale3D, TransformMat3x3, Translation3D},
    datatypes,
};

use super::Transform3D;

/// A 3D rotation.
///
/// This is *not* a component, but a helper type for populating `Transform3D` with rotations.
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Rotation3D {
    /// Rotation defined by a quaternion.
    Quaternion(RotationQuat),

    /// Rotation defined with an axis and an angle.
    AxisAngle(RotationAxisAngle),
}

impl From<RotationQuat> for Rotation3D {
    #[inline]
    fn from(quat: RotationQuat) -> Self {
        Self::Quaternion(quat)
    }
}

impl From<RotationAxisAngle> for Rotation3D {
    #[inline]
    fn from(axis_angle: RotationAxisAngle) -> Self {
        Self::AxisAngle(axis_angle)
    }
}

impl From<datatypes::Quaternion> for Rotation3D {
    #[inline]
    fn from(quat: datatypes::Quaternion) -> Self {
        Self::Quaternion(quat.into())
    }
}

impl From<datatypes::RotationAxisAngle> for Rotation3D {
    #[inline]
    fn from(axis_angle: datatypes::RotationAxisAngle) -> Self {
        Self::AxisAngle(axis_angle.into())
    }
}

impl Transform3D {
    /// Convenience method that takes any kind of (single) rotation representation and sets it on this transform.
    #[inline]
    pub fn with_rotation(self, rotation: impl Into<Rotation3D>) -> Self {
        match rotation.into() {
            Rotation3D::Quaternion(quaternion) => Self {
                quaternion: Some(vec![quaternion]),
                ..self
            },
            Rotation3D::AxisAngle(rotation_axis_angle) => Self {
                rotation_axis_angle: Some(vec![rotation_axis_angle]),
                ..self
            },
        }
    }

    /// From a translation.
    #[inline]
    pub fn from_translation(translation: impl Into<Translation3D>) -> Self {
        Self {
            translation: Some(vec![translation.into()]),
            ..Self::default()
        }
    }

    /// From a translation.
    #[inline]
    pub fn from_mat3x3(mat3x3: impl Into<TransformMat3x3>) -> Self {
        Self {
            mat3x3: Some(vec![mat3x3.into()]),
            ..Self::default()
        }
    }

    /// From a rotation
    #[inline]
    pub fn from_rotation(rotation: impl Into<Rotation3D>) -> Self {
        Self::default().with_rotation(rotation)
    }

    /// From a scale
    #[inline]
    pub fn from_scale(scale: impl Into<Scale3D>) -> Self {
        Self {
            scale: Some(vec![scale.into()]),
            ..Self::default()
        }
    }

    /// From a translation applied after a rotation, known as a rigid transformation.
    #[inline]
    pub fn from_translation_rotation(
        translation: impl Into<Translation3D>,
        rotation: impl Into<Rotation3D>,
    ) -> Self {
        Self {
            translation: Some(vec![translation.into()]),
            ..Self::default()
        }
        .with_rotation(rotation)
    }

    /// From a translation applied after a 3x3 matrix.
    #[inline]
    pub fn from_translation_mat3x3(
        translation: impl Into<Translation3D>,
        mat3x3: impl Into<TransformMat3x3>,
    ) -> Self {
        Self {
            mat3x3: Some(vec![mat3x3.into()]),
            translation: Some(vec![translation.into()]),
            ..Self::default()
        }
    }

    /// From a translation applied after a scale.
    #[inline]
    pub fn from_translation_scale(
        translation: impl Into<Translation3D>,
        scale: impl Into<Scale3D>,
    ) -> Self {
        Self {
            scale: Some(vec![scale.into()]),
            translation: Some(vec![translation.into()]),
            ..Self::default()
        }
    }

    /// From a translation, applied after a rotation & scale, known as an affine transformation.
    #[inline]
    pub fn from_translation_rotation_scale(
        translation: impl Into<Translation3D>,
        rotation: impl Into<Rotation3D>,
        scale: impl Into<Scale3D>,
    ) -> Self {
        Self {
            scale: Some(vec![scale.into()]),
            translation: Some(vec![translation.into()]),
            ..Self::default()
        }
        .with_rotation(rotation)
    }

    /// From a rotation & scale
    #[inline]
    pub fn from_rotation_scale(rotation: impl Into<Rotation3D>, scale: impl Into<Scale3D>) -> Self {
        Self {
            scale: Some(vec![scale.into()]),
            ..Self::default()
        }
        .with_rotation(rotation)
    }

    /// Indicate that this transform is from parent to child.
    /// This is the oppositve of the default, which is from child to parent.
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn from_parent(mut self) -> Self {
        self.transform = self.transform.from_parent();
        self
    }
}
