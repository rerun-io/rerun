use super::Transform3D;
use crate::Rotation3D;
use crate::components::{Scale3D, TransformMat3x3, Translation3D};

impl Transform3D {
    /// The identity transform.
    ///
    /// This is the same as [`Self::clear_fields`], i.e. it logs an empty (default)
    /// value for all components.
    pub const IDENTITY: Self = Self {
        translation: None,
        rotation_axis_angle: None,
        quaternion: None,
        scale: None,
        mat3x3: None,
        relation: None,
        child_frame: None,
        parent_frame: None,
    };

    /// Convenience method that takes any kind of (single) rotation representation and sets it on this transform.
    #[inline]
    pub fn with_rotation(self, rotation: impl Into<Rotation3D>) -> Self {
        match rotation.into() {
            Rotation3D::Quaternion(quaternion) => self.with_quaternion(quaternion),
            Rotation3D::AxisAngle(rotation_axis_angle) => {
                self.with_rotation_axis_angle(rotation_axis_angle)
            }
        }
    }

    /// From a translation, clearing all other fields.
    #[inline]
    pub fn from_translation(translation: impl Into<Translation3D>) -> Self {
        Self::new().with_translation(translation)
    }

    /// From a 3x3 matrix, clearing all other fields.
    #[inline]
    pub fn from_mat3x3(mat3x3: impl Into<TransformMat3x3>) -> Self {
        Self::new().with_mat3x3(mat3x3)
    }

    /// From a rotation, clearing all other fields.
    #[inline]
    pub fn from_rotation(rotation: impl Into<Rotation3D>) -> Self {
        Self::new().with_rotation(rotation)
    }

    /// From a scale, clearing all other fields.
    #[inline]
    pub fn from_scale(scale: impl Into<Scale3D>) -> Self {
        Self::new().with_scale(scale)
    }

    /// From a translation applied after a rotation, known as a rigid transformation.
    ///
    /// Clears all other fields.
    #[inline]
    pub fn from_translation_rotation(
        translation: impl Into<Translation3D>,
        rotation: impl Into<Rotation3D>,
    ) -> Self {
        Self::new()
            .with_translation(translation)
            .with_rotation(rotation)
    }

    /// From a translation applied after a 3x3 matrix, clearing all other fields.
    #[inline]
    pub fn from_translation_mat3x3(
        translation: impl Into<Translation3D>,
        mat3x3: impl Into<TransformMat3x3>,
    ) -> Self {
        Self::new()
            .with_mat3x3(mat3x3)
            .with_translation(translation)
    }

    /// From a translation applied after a scale, clearing all other fields.
    #[inline]
    pub fn from_translation_scale(
        translation: impl Into<Translation3D>,
        scale: impl Into<Scale3D>,
    ) -> Self {
        Self::new().with_scale(scale).with_translation(translation)
    }

    /// From a translation, applied after a rotation & scale, known as an affine transformation, clearing all other fields.
    #[inline]
    pub fn from_translation_rotation_scale(
        translation: impl Into<Translation3D>,
        rotation: impl Into<Rotation3D>,
        scale: impl Into<Scale3D>,
    ) -> Self {
        Self::new()
            .with_scale(scale)
            .with_translation(translation)
            .with_rotation(rotation)
    }

    /// From a rotation & scale, clearing all other fields.
    #[inline]
    pub fn from_rotation_scale(rotation: impl Into<Rotation3D>, scale: impl Into<Scale3D>) -> Self {
        Self::new().with_rotation(rotation).with_scale(scale)
    }
}
