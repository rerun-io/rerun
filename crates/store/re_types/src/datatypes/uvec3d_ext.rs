use super::UVec3D;

impl UVec3D {
    /// The zero vector, i.e. the additive identity.
    pub const ZERO: Self = Self([0; 3]);

    /// The unit vector `[1, 1, 1]`, i.e. the multiplicative identity.
    pub const ONE: Self = Self([1; 3]);

    /// Create a new vector.
    #[inline]
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self([x, y, z])
    }

    /// The x-coordinate, i.e. index 0.
    #[inline]
    pub fn x(&self) -> u32 {
        self.0[0]
    }

    /// The y-coordinate, i.e. index 1.
    #[inline]
    pub fn y(&self) -> u32 {
        self.0[1]
    }

    /// The z-coordinate, i.e. index 2.
    #[inline]
    pub fn z(&self) -> u32 {
        self.0[2]
    }
}

impl From<(u32, u32, u32)> for UVec3D {
    #[inline]
    fn from((x, y, z): (u32, u32, u32)) -> Self {
        Self::new(x, y, z)
    }
}

// NOTE: All these by-ref impls make the lives of end-users much easier when juggling around with
// slices, because Rust cannot keep track of the inherent `Copy` capability of it all across all the
// layers of `Into`/`IntoIterator`.

impl<'a> From<&'a Self> for UVec3D {
    fn from(v: &'a Self) -> Self {
        Self(v.0)
    }
}

impl<'a> From<&'a (u32, u32, u32)> for UVec3D {
    #[inline]
    fn from((x, y, z): &'a (u32, u32, u32)) -> Self {
        Self::new(*x, *y, *z)
    }
}

impl<'a> From<&'a [u32; 3]> for UVec3D {
    #[inline]
    fn from(v: &'a [u32; 3]) -> Self {
        Self(*v)
    }
}

impl<Idx> std::ops::Index<Idx> for UVec3D
where
    Idx: std::slice::SliceIndex<[u32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

#[cfg(feature = "glam")]
impl From<UVec3D> for glam::UVec3 {
    #[inline]
    fn from(v: UVec3D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::UVec3> for UVec3D {
    #[inline]
    fn from(v: glam::UVec3) -> Self {
        Self(v.to_array())
    }
}

impl std::fmt::Display for UVec3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}, {}]", self.x(), self.y(), self.z(),)
    }
}
