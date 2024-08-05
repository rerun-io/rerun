use crate::{
    components::{Scale3D, TransformMat3x3, TransformRelation, Translation3D},
    Rotation3D,
};

use super::Transform3D;

impl Transform3D {
    /// Convenience method that takes any kind of (single) rotation representation and sets it on this transform.
    #[inline]
    pub fn with_rotation(self, rotation: impl Into<Rotation3D>) -> Self {
        match rotation.into() {
            Rotation3D::Quaternion(quaternion) => Self {
                quaternion: Some(quaternion),
                ..self
            },
            Rotation3D::AxisAngle(rotation_axis_angle) => Self {
                rotation_axis_angle: Some(rotation_axis_angle),
                ..self
            },
        }
    }

    /// From a translation.
    #[inline]
    pub fn from_translation(translation: impl Into<Translation3D>) -> Self {
        Self {
            translation: Some(translation.into()),
            ..Self::default()
        }
    }

    /// From a translation.
    #[inline]
    pub fn from_mat3x3(mat3x3: impl Into<TransformMat3x3>) -> Self {
        Self {
            mat3x3: Some(mat3x3.into()),
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
            scale: Some(scale.into()),
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
            translation: Some(translation.into()),
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
            mat3x3: Some(mat3x3.into()),
            translation: Some(translation.into()),
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
            scale: Some(scale.into()),
            translation: Some(translation.into()),
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
            scale: Some(scale.into()),
            translation: Some(translation.into()),
            ..Self::default()
        }
        .with_rotation(rotation)
    }

    /// From a rotation & scale
    #[inline]
    pub fn from_rotation_scale(rotation: impl Into<Rotation3D>, scale: impl Into<Scale3D>) -> Self {
        Self {
            scale: Some(scale.into()),
            ..Self::default()
        }
        .with_rotation(rotation)
    }

    /// Indicate that this transform is from parent to child.
    ///
    /// This is the opposite of the default, which is from child to parent.
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    #[deprecated(
        since = "0.18.0",
        note = "Use `.with_relation(rerun::TransformRelation::ChildFromParent)` instead."
    )]
    pub fn from_parent(self) -> Self {
        self.with_relation(TransformRelation::ChildFromParent)
    }
}
