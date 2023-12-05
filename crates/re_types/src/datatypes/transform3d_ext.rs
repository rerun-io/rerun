use super::{Scale3D, Transform3D};

use crate::datatypes::{
    Rotation3D, RotationAxisAngle, TranslationAndMat3x3, TranslationRotationScale3D, Vec3D,
};

impl Transform3D {
    pub const IDENTITY: Self = Self::TranslationRotationScale(TranslationRotationScale3D::IDENTITY);

    /// From a translation.
    #[inline]
    pub fn from_translation(translation: impl Into<Vec3D>) -> Self {
        Self::from(TranslationRotationScale3D::from_translation(translation))
    }

    /// From a translation applied after a rotation, known as a rigid transformation.
    #[inline]
    pub fn from_translation_rotation(
        translation: impl Into<Vec3D>,
        rotation: impl Into<Rotation3D>,
    ) -> Self {
        Self::from(TranslationRotationScale3D::from_translation_rotation(
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
        Self::from(TranslationAndMat3x3::new(translation, mat3x3))
    }

    /// From a translation, applied after a rotation & scale, known as an affine transformation.
    #[inline]
    pub fn from_translation_rotation_scale(
        translation: impl Into<Vec3D>,
        rotation: impl Into<Rotation3D>,
        scale: impl Into<Scale3D>,
    ) -> Self {
        Self::from(TranslationRotationScale3D::from_translation_rotation_scale(
            translation,
            rotation,
            scale,
        ))
    }

    /// From a rotation & scale
    #[inline]
    pub fn from_rotation_scale(rotation: impl Into<Rotation3D>, scale: impl Into<Scale3D>) -> Self {
        Self::from(TranslationRotationScale3D::from_rotation_scale(
            rotation, scale,
        ))
    }

    /// Indicate that this transform is from parent to child.
    /// This is the oppositve of the default, which is from child to parent.
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn from_parent(mut self) -> Self {
        match &mut self {
            Self::TranslationAndMat3x3(t) => {
                t.from_parent = true;
            }
            Self::TranslationRotationScale(t) => {
                t.from_parent = true;
            }
        }
        self
    }

    /// Indicates whether this transform is from parent to child.
    /// This is the oppositve of the default, which is from child to parent.
    #[inline]
    #[allow(clippy::wrong_self_convention)]
    pub fn is_from_parent(&self) -> bool {
        match self {
            Self::TranslationAndMat3x3(t) => t.from_parent,
            Self::TranslationRotationScale(t) => t.from_parent,
        }
    }
}

impl Default for Transform3D {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<TranslationAndMat3x3> for Transform3D {
    #[inline]
    fn from(v: TranslationAndMat3x3) -> Self {
        Self::TranslationAndMat3x3(v)
    }
}

impl From<TranslationRotationScale3D> for Transform3D {
    #[inline]
    fn from(v: TranslationRotationScale3D) -> Self {
        Self::TranslationRotationScale(v)
    }
}

impl From<Vec3D> for Transform3D {
    #[inline]
    fn from(v: Vec3D) -> Self {
        Self::TranslationRotationScale(v.into())
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for Transform3D {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Self::TranslationRotationScale(v.into())
    }
}

impl From<RotationAxisAngle> for Transform3D {
    #[inline]
    fn from(v: RotationAxisAngle) -> Self {
        let rotation = Rotation3D::from(v);
        Self::TranslationRotationScale(rotation.into())
    }
}

#[cfg(feature = "glam")]
impl From<Transform3D> for glam::Affine3A {
    fn from(value: Transform3D) -> Self {
        match value {
            Transform3D::TranslationAndMat3x3(TranslationAndMat3x3 {
                translation,
                mat3x3,
                from_parent: _,
            }) => glam::Affine3A::from_mat3_translation(
                mat3x3.unwrap_or(super::Mat3x3::IDENTITY).into(),
                translation.map_or(glam::Vec3::ZERO, |v| v.into()),
            ),

            Transform3D::TranslationRotationScale(TranslationRotationScale3D {
                translation,
                rotation,
                scale,
                from_parent: _,
            }) => glam::Affine3A::from_scale_rotation_translation(
                scale.map_or(glam::Vec3::ONE, |s| s.into()),
                rotation.map_or(glam::Quat::IDENTITY, |q| q.into()),
                translation.map_or(glam::Vec3::ZERO, |v| v.into()),
            ),
        }
    }
}
