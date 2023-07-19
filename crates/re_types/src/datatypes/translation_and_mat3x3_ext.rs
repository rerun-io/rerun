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
        matrix: None,
        from_parent: false,
    };

    /// Create a new `TranslationAndMat3`.
    #[inline]
    pub fn new<T: Into<Vec3D>, M: Into<Mat3x3>>(translation: T, matrix: M) -> Self {
        Self {
            translation: Some(translation.into()),
            matrix: Some(matrix.into()),
            from_parent: false,
        }
    }

    #[inline]
    pub fn translation<T: Into<Vec3D>>(translation: T) -> Self {
        Self {
            translation: Some(translation.into()),
            matrix: None,
            from_parent: false,
        }
    }

    #[inline]
    pub fn rotation<M: Into<Mat3x3>>(matrix: M) -> Self {
        Self {
            translation: None,
            matrix: Some(matrix.into()),
            from_parent: false,
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn from_parent(mut self) -> Self {
        self.from_parent = true;
        self
    }
}
