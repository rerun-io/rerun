use super::Quaternion;

// ---

impl Default for Quaternion {
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Quaternion {
    pub const IDENTITY: Self = Self([0.0, 0.0, 0.0, 1.0]);

    #[inline]
    pub const fn from_xyzw(xyzw: [f32; 4]) -> Self {
        Self(xyzw)
    }

    #[inline]
    pub const fn from_wxyz([w, x, y, z]: [f32; 4]) -> Self {
        Self([x, y, z, w])
    }
}

#[cfg(feature = "glam")]
impl From<Quaternion> for glam::Quat {
    #[inline]
    fn from(q: Quaternion) -> Self {
        let [x, y, z, w] = q.0;
        Self::from_xyzw(x, y, z, w).normalize()
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
