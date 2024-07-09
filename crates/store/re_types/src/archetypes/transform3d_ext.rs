use crate::{
    components::{TransformMat3x3, Translation3D},
    datatypes::{Rotation3D, Scale3D, TranslationAndMat3x3, TranslationRotationScale3D},
};

use super::Transform3D;

impl Transform3D {
    /// Identity transform, i.e. parent & child are in the same space.
    pub const IDENTITY: Self = Self {
        transform: crate::components::Transform3D::IDENTITY,
        mat3x3: None,
        translation: None,
        axis_length: None,
    };

    /// From a translation.
    #[inline]
    pub fn from_translation(translation: impl Into<Translation3D>) -> Self {
        Self {
            translation: Some(vec![translation.into()]),
            ..Self::IDENTITY
        }
    }

    /// From a translation.
    #[inline]
    pub fn from_mat3x3(mat3x3: impl Into<TransformMat3x3>) -> Self {
        Self {
            mat3x3: Some(vec![mat3x3.into()]),
            ..Self::IDENTITY
        }
    }

    /// From a rotation
    #[inline]
    pub fn from_rotation(rotation: impl Into<Rotation3D>) -> Self {
        Self {
            transform: TranslationRotationScale3D::from_rotation(rotation).into(),
            ..Self::IDENTITY
        }
    }

    /// From a scale
    #[inline]
    pub fn from_scale(scale: impl Into<Scale3D>) -> Self {
        Self {
            transform: TranslationRotationScale3D::from_scale(scale).into(),
            ..Self::IDENTITY
        }
    }

    /// From a translation applied after a rotation, known as a rigid transformation.
    #[inline]
    pub fn from_translation_rotation(
        translation: impl Into<Translation3D>,
        rotation: impl Into<Rotation3D>,
    ) -> Self {
        Self {
            transform: TranslationRotationScale3D::from_rotation(rotation).into(),
            translation: Some(vec![translation.into()]),
            ..Self::IDENTITY
        }
    }

    /// From a translation applied after a 3x3 matrix.
    #[inline]
    pub fn from_translation_mat3x3(
        translation: impl Into<Translation3D>,
        mat3x3: impl Into<crate::datatypes::Mat3x3>,
    ) -> Self {
        Self {
            transform: TranslationAndMat3x3::from_mat3x3(mat3x3).into(),

            translation: Some(vec![translation.into()]),
            ..Self::IDENTITY
        }
    }

    /// From a translation applied after a scale
    #[inline]
    pub fn from_translation_scale(
        translation: impl Into<Translation3D>,
        scale: impl Into<Scale3D>,
    ) -> Self {
        Self {
            transform: TranslationRotationScale3D::from_scale(scale).into(),
            translation: Some(vec![translation.into()]),
            ..Self::IDENTITY
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
            transform: TranslationRotationScale3D::from_rotation_scale(rotation, scale).into(),
            translation: Some(vec![translation.into()]),
            ..Self::IDENTITY
        }
    }

    /// From a rotation & scale
    #[inline]
    pub fn from_rotation_scale(rotation: impl Into<Rotation3D>, scale: impl Into<Scale3D>) -> Self {
        Self {
            transform: TranslationRotationScale3D::from_rotation_scale(rotation, scale).into(),
            ..Self::IDENTITY
        }
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
