use crate::datatypes::Vec3D;

use super::Arrow3D;

impl Arrow3D {
    pub fn new(origin: impl Into<Vec3D>, vector: impl Into<Vec3D>) -> Self {
        Self(crate::datatypes::Arrow3D::new(origin, vector))
    }
}
