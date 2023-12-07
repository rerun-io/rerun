use crate::datatypes::{Mat3x3, Vec3D};

use super::TranslationAndMat3x3;

impl Default for TranslationAndMat3x3 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl TranslationAndMat3x3 {
    pub const IDENTITY: Self = Self {
        translation: None,
        mat3x3: None,
        from_parent: false,
    };

    /// Create a new `TranslationAndMat3`.
    #[inline]
    pub fn new<T: Into<Vec3D>, M: Into<Mat3x3>>(translation: T, mat3x3: M) -> Self {
        Self {
            translation: Some(translation.into()),
            mat3x3: Some(mat3x3.into()),
            from_parent: false,
        }
    }

    #[inline]
    pub fn from_translation<T: Into<Vec3D>>(translation: T) -> Self {
        Self {
            translation: Some(translation.into()),
            mat3x3: None,
            from_parent: false,
        }
    }

    #[inline]
    pub fn from_mat3x3<M: Into<Mat3x3>>(mat3x3: M) -> Self {
        Self {
            translation: None,
            mat3x3: Some(mat3x3.into()),
            from_parent: false,
        }
    }

    /// Indicate that this transform is from parent to child.
    /// This is the oppositve of the default, which is from child to parent.
    #[allow(clippy::wrong_self_convention)]
    #[inline]
    pub fn from_parent(mut self) -> Self {
        self.from_parent = true;
        self
    }
}
