use super::RotationQuat;

impl RotationQuat {
    /// The identity rotation, representing no rotation.
    ///
    /// Keep in mind that logging an identity rotation is different from logging no rotation at all
    /// in thus far that it will write data to the store.
    pub const IDENTITY: Self = Self(crate::datatypes::Quaternion::IDENTITY);
}

#[cfg(feature = "glam")]
impl From<RotationQuat> for glam::Affine3A {
    #[inline]
    fn from(val: RotationQuat) -> Self {
        Self::from_quat(val.0.into())
    }
}
