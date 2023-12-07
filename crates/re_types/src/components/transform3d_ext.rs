use super::Transform3D;

use crate::datatypes::{
    Rotation3D, Scale3D, Transform3D as Transform3DDatatype, TranslationAndMat3x3,
    TranslationRotationScale3D, Vec3D,
};

impl Transform3D {
    /// Identity transform, i.e. parent & child are in the same space.
    pub const IDENTITY: Self = Self(Transform3DDatatype::IDENTITY);

    /// Creates a new transform with a given representation, transforming from the parent space into the child space.
    #[inline]
    pub fn new<T: Into<Transform3DDatatype>>(t: T) -> Self {
        Self(t.into())
    }

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
        self.0 = self.0.from_parent();
        self
    }
}

#[cfg(feature = "glam")]
impl Transform3D {
    #[inline]
    pub fn into_parent_from_child_transform(self) -> glam::Affine3A {
        let transform: glam::Affine3A = self.0.into();
        if self.0.is_from_parent() {
            transform.inverse()
        } else {
            transform
        }
    }

    #[inline]
    pub fn into_child_from_parent_transform(self) -> glam::Affine3A {
        let transform: glam::Affine3A = self.0.into();
        if self.0.is_from_parent() {
            transform
        } else {
            transform.inverse()
        }
    }
}
