use super::Quaternion;

// ---

impl Default for Quaternion {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Quaternion {
    /// The identity quaternion representing no rotation.
    pub const IDENTITY: Self = Self([0.0, 0.0, 0.0, 1.0]);

    /// A quaternion that represents an invalid transform.
    pub const INVALID: Self = Self([0.0, 0.0, 0.0, 0.0]);

    /// From XYZW.
    #[inline]
    pub const fn from_xyzw(xyzw: [f32; 4]) -> Self {
        Self(xyzw)
    }

    /// From WXYZ.
    #[inline]
    pub const fn from_wxyz([w, x, y, z]: [f32; 4]) -> Self {
        Self([x, y, z, w])
    }

    /// The components of the quaternion in X,Y,Z,W order.
    #[inline]
    pub fn xyzw(&self) -> [f32; 4] {
        self.0
    }
}

#[cfg(feature = "glam")]
impl TryFrom<Quaternion> for glam::Quat {
    type Error = ();

    #[inline]
    fn try_from(q: Quaternion) -> Result<Self, ()> {
        glam::Vec4::from(q.0)
            .try_normalize()
            .map(Self::from_vec4)
            .ok_or(())
    }
}

#[cfg(feature = "glam")]
impl From<glam::Quat> for Quaternion {
    #[inline]
    fn from(q: glam::Quat) -> Self {
        Self::from_xyzw(q.to_array())
    }
}

#[cfg(feature = "mint")]
impl From<Quaternion> for mint::Quaternion<f32> {
    #[inline]
    fn from(val: Quaternion) -> Self {
        val.0.into()
    }
}

#[cfg(feature = "mint")]
impl From<mint::Quaternion<f32>> for Quaternion {
    #[inline]
    fn from(val: mint::Quaternion<f32>) -> Self {
        Self::from_xyzw(val.into())
    }
}
