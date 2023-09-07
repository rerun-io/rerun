use super::Transform3D;

use crate::datatypes::{
    Rotation3D, RotationAxisAngle, TranslationAndMat3x3, TranslationRotationScale3D, Vec3D,
};

impl Transform3D {
    pub const IDENTITY: Self = Self::TranslationRotationScale(TranslationRotationScale3D::IDENTITY);

    #[inline]
    #[allow(clippy::wrong_self_convention)]
    pub fn from_parent(&self) -> bool {
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
                matrix,
                from_parent: _,
            }) => glam::Affine3A::from_mat3_translation(
                matrix.unwrap_or_default().into(),
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
