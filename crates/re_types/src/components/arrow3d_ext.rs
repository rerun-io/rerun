use crate::datatypes::Vec3D;

use super::Arrow3D;

impl Arrow3D {
    #[inline]
    pub fn new(origin: impl Into<Vec3D>, vector: impl Into<Vec3D>) -> Self {
        Self(crate::datatypes::Arrow3D::new(origin, vector))
    }

    #[inline]
    pub fn origin(&self) -> Vec3D {
        self.0.origin
    }

    #[inline]
    pub fn vector(&self) -> Vec3D {
        self.0.vector
    }
}
