use super::LeafRotationQuat;

impl LeafRotationQuat {
    /// The identity rotation, representing no rotation.
    pub const IDENTITY: Self = Self(crate::datatypes::Quaternion::IDENTITY);
}

#[cfg(feature = "glam")]
impl From<LeafRotationQuat> for glam::Affine3A {
    #[inline]
    fn from(val: LeafRotationQuat) -> Self {
        Self::from_quat(val.0.into())
    }
}
