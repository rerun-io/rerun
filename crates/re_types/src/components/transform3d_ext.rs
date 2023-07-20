use super::Transform3D;

use crate::datatypes::{
    RotationAxisAngle, Transform3D as Transform3DRepr, TranslationAndMat3x3,
    TranslationRotationScale3D,
};

impl Transform3D {
    /// Identity transform, i.e. parent & child are in the same space.
    pub const IDENTITY: Self = Self(Transform3DRepr::IDENTITY);

    /// Creates a new transform with a given representation, transforming from the parent space into the child space.
    pub fn new<T: Into<Transform3DRepr>>(t: T) -> Self {
        Self(t.into())
    }
}

#[cfg(feature = "glam")]
impl Transform3D {
    #[inline]
    pub fn into_parent_from_child_transform(self) -> glam::Affine3A {
        let transform: glam::Affine3A = self.0.into();
        if self.0.from_parent() {
            transform.inverse()
        } else {
            transform
        }
    }

    #[inline]
    pub fn into_child_from_parent_transform(self) -> glam::Affine3A {
        let transform: glam::Affine3A = self.0.into();
        if self.0.from_parent() {
            transform
        } else {
            transform.inverse()
        }
    }
}

impl From<Transform3DRepr> for Transform3D {
    fn from(value: Transform3DRepr) -> Self {
        Self(value)
    }
}

impl From<RotationAxisAngle> for Transform3D {
    fn from(value: RotationAxisAngle) -> Self {
        Self(value.into())
    }
}

impl From<TranslationRotationScale3D> for Transform3D {
    fn from(value: TranslationRotationScale3D) -> Self {
        Self(value.into())
    }
}

impl From<TranslationAndMat3x3> for Transform3D {
    fn from(value: TranslationAndMat3x3) -> Self {
        Self(value.into())
    }
}
