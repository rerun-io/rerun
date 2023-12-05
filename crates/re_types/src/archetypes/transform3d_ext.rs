use crate::datatypes::{
    Rotation3D, Scale3D, TranslationAndMat3x3, TranslationRotationScale3D, Vec3D,
};

use super::Transform3D;

impl Transform3D {
    /// Identity transform, i.e. parent & child are in the same space.
    pub const IDENTITY: Self = Self {
        transform: crate::components::Transform3D::IDENTITY,
    };

    /// From a translation.
    #[inline]
    pub fn from_translation(translation: impl Into<Vec3D>) -> Self {
        Self::new(TranslationRotationScale3D::from_translation(translation))
    }

    /// From a rotation
    #[inline]
    pub fn from_rotation(rotation: impl Into<Rotation3D>) -> Self {
        Self::new(TranslationRotationScale3D::from_rotation(rotation))
    }

    /// From a scale
    #[inline]
    pub fn from_scale(scale: impl Into<Scale3D>) -> Self {
        Self::new(TranslationRotationScale3D::from_scale(scale))
    }

    /// From a translation applied after a rotation, known as a rigid transformation.
    #[inline]
    pub fn from_translation_rotation(
        translation: impl Into<Vec3D>,
        rotation: impl Into<Rotation3D>,
    ) -> Self {
        Self::new(TranslationRotationScale3D::from_translation_rotation(
            translation,
            rotation,
        ))
    }

    /// From a translation applied after a 3x3 matrix.
    #[inline]
    pub fn from_translation_mat3x3(
        translation: impl Into<Vec3D>,
        mat3x3: impl Into<crate::datatypes::Mat3x3>,
    ) -> Self {
        Self::new(TranslationAndMat3x3::new(translation, mat3x3))
    }

    /// From a translation, applied after a rotation & scale, known as an affine transformation.
    #[inline]
    pub fn from_translation_rotation_scale(
        translation: impl Into<Vec3D>,
        rotation: impl Into<Rotation3D>,
        scale: impl Into<Scale3D>,
    ) -> Self {
        Self::new(TranslationRotationScale3D::from_translation_rotation_scale(
            translation,
            rotation,
            scale,
        ))
    }

    /// From a rotation & scale
    #[inline]
    pub fn from_rotation_scale(rotation: impl Into<Rotation3D>, scale: impl Into<Scale3D>) -> Self {
        Self::new(TranslationRotationScale3D::from_rotation_scale(
            rotation, scale,
        ))
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
