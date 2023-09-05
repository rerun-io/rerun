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
    pub fn translation<T: Into<Vec3D>>(translation: T) -> Self {
        Self {
            translation: Some(translation.into()),
            rotation: None,
            scale: None,
            from_parent: false,
        }
    }

    /// From a translation applied after a rotation, known as a rigid transformation.
    #[inline]
    pub fn rigid<T: Into<Vec3D>, R: Into<Rotation3D>>(translation: T, rotation: R) -> Self {
        Self {
            translation: Some(translation.into()),
            rotation: Some(rotation.into()),
            scale: None,
            from_parent: false,
        }
    }

    /// From a translation, applied after a rotation & scale, known as an affine transformation.
    #[inline]
    pub fn affine<T: Into<Vec3D>, R: Into<Rotation3D>, S: Into<Scale3D>>(
        translation: T,
        rotation: R,
        scale: S,
    ) -> Self {
        Self {
            translation: Some(translation.into()),
            rotation: Some(rotation.into()),
            scale: Some(scale.into()),
            from_parent: false,
        }
    }

    #[inline]
    #[allow(clippy::wrong_self_convention)]
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
