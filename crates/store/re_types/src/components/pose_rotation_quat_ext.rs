use super::PoseRotationQuat;

impl PoseRotationQuat {
    /// The identity rotation, representing no rotation.
    pub const IDENTITY: Self = Self(crate::datatypes::Quaternion::IDENTITY);
}

#[cfg(feature = "glam")]
impl From<PoseRotationQuat> for glam::Affine3A {
    #[inline]
    fn from(val: PoseRotationQuat) -> Self {
        Self::from_quat(val.0.into())
    }
}
