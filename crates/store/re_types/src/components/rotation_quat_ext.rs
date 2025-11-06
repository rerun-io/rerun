use super::RotationQuat;

impl RotationQuat {
    /// The identity rotation, representing no rotation.
    ///
    /// Keep in mind that logging an identity rotation is different from logging no rotation at all
    /// in thus far that it will write data to the store.
    pub const IDENTITY: Self = Self(crate::datatypes::Quaternion::IDENTITY);

    /// A rotation that represents an invalid transform.
    pub const INVALID: Self = Self(crate::datatypes::Quaternion::INVALID);
}

#[cfg(feature = "glam")]
impl TryFrom<RotationQuat> for glam::Affine3A {
    type Error = ();

    #[inline]
    fn try_from(val: RotationQuat) -> Result<Self, Self::Error> {
        Ok(Self::from_quat(glam::Quat::try_from(val.0)?))
    }
}
