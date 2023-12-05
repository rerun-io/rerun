use super::TranslationRotationScale3D;

use crate::datatypes::{Rotation3D, Scale3D, Vec3D};

impl TranslationRotationScale3D {
    pub const IDENTITY: Self = Self {
        translation: None,
        rotation: None,
        scale: None,
        from_parent: false,
    };

    /// From a translation.
    #[inline]
    pub fn from_translation(translation: impl Into<Vec3D>) -> Self {
        Self {
            translation: Some(translation.into()),
            rotation: None,
            scale: None,
            from_parent: false,
        }
    }

    /// From a rotation
    #[inline]
    pub fn from_rotation(rotation: impl Into<Rotation3D>) -> Self {
        Self {
            translation: None,
            rotation: Some(rotation.into()),
            scale: None,
            from_parent: false,
        }
    }

    /// From a rotation & scale
    #[inline]
    pub fn from_scale(scale: impl Into<Scale3D>) -> Self {
        Self {
            translation: None,
            rotation: None,
            scale: Some(scale.into()),
            from_parent: false,
        }
    }

    /// From a translation applied after a rotation, known as a rigid transformation.
    #[inline]
    pub fn from_translation_rotation(
        translation: impl Into<Vec3D>,
        rotation: impl Into<Rotation3D>,
    ) -> Self {
        Self {
            translation: Some(translation.into()),
            rotation: Some(rotation.into()),
            scale: None,
            from_parent: false,
        }
    }

    /// From a rotation & scale
    #[inline]
    pub fn from_rotation_scale(rotation: impl Into<Rotation3D>, scale: impl Into<Scale3D>) -> Self {
        Self {
            translation: None,
            rotation: Some(rotation.into()),
            scale: Some(scale.into()),
            from_parent: false,
        }
    }

    /// From a translation, applied after a rotation & scale, known as an affine transformation.
    #[inline]
    pub fn from_translation_rotation_scale(
        translation: impl Into<Vec3D>,
        rotation: impl Into<Rotation3D>,
        scale: impl Into<Scale3D>,
    ) -> Self {
        Self {
            translation: Some(translation.into()),
            rotation: Some(rotation.into()),
            scale: Some(scale.into()),
            from_parent: false,
        }
    }

    /// Indicate that this transform is from parent to child.
    /// This is the opposite of the default, which is from child to parent.
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn from_parent(mut self) -> Self {
        self.from_parent = true;
        self
    }
}

impl Default for TranslationRotationScale3D {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<Vec3D> for TranslationRotationScale3D {
    #[inline]
    fn from(v: Vec3D) -> Self {
        Self {
            translation: Some(v),
            ..Default::default()
        }
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec3> for TranslationRotationScale3D {
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Self {
            translation: Some(v.into()),
            ..Default::default()
        }
    }
}

impl From<Rotation3D> for TranslationRotationScale3D {
    #[inline]
    fn from(v: Rotation3D) -> Self {
        Self {
            rotation: Some(v),
            ..Default::default()
        }
    }
}

impl From<Scale3D> for TranslationRotationScale3D {
    #[inline]
    fn from(v: Scale3D) -> Self {
        Self {
            scale: Some(v),
            ..Default::default()
        }
    }
}
