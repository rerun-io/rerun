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
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self([x, y, z, w])
    }

    #[inline]
    pub const fn from_xyzw(xyzw: [f32; 4]) -> Self {
        Self(xyzw)
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
        let (x, y, z, w) = q.into();
        Self::new(x, y, z, w)
    }
}
