use super::IVec3D;

impl IVec3D {
    /// The zero vector, i.e. the additive identity.
    pub const ZERO: Self = Self([0; 3]);

    /// The unit vector `[1, 1, 1]`, i.e. the multiplicative identity.
    pub const ONE: Self = Self([1; 3]);

    /// Create a new vector.
    #[inline]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self([x, y, z])
    }

    /// The x-coordinate, i.e. index 0.
    #[inline]
    pub fn x(&self) -> i32 {
        self.0[0]
    }

    /// The y-coordinate, i.e. index 1.
    #[inline]
    pub fn y(&self) -> i32 {
        self.0[1]
    }

    /// The z-coordinate, i.e. index 2.
    #[inline]
    pub fn z(&self) -> i32 {
        self.0[2]
    }
}

impl From<(i32, i32, i32)> for IVec3D {
    #[inline]
    fn from((x, y, z): (i32, i32, i32)) -> Self {
        Self::new(x, y, z)
    }
}

// NOTE: All these by-ref impls make the lives of end-users much easier when juggling around with
// slices, because Rust cannot keep track of the inherent `Copy` capability of it all across all the
// layers of `Into`/`IntoIterator`.

impl<'a> From<&'a Self> for IVec3D {
    fn from(v: &'a Self) -> Self {
        Self(v.0)
    }
}

impl<'a> From<&'a (i32, i32, i32)> for IVec3D {
    #[inline]
    fn from((x, y, z): &'a (i32, i32, i32)) -> Self {
        Self::new(*x, *y, *z)
    }
}

impl<'a> From<&'a [i32; 3]> for IVec3D {
    #[inline]
    fn from(v: &'a [i32; 3]) -> Self {
        Self(*v)
    }
}

impl<Idx> std::ops::Index<Idx> for IVec3D
where
    Idx: std::slice::SliceIndex<[i32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

#[cfg(feature = "glam")]
impl From<IVec3D> for glam::IVec3 {
    #[inline]
    fn from(v: IVec3D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::IVec3> for IVec3D {
    #[inline]
    fn from(v: glam::IVec3) -> Self {
        Self(v.to_array())
    }
}

impl std::fmt::Display for IVec3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}, {}]", self.x(), self.y(), self.z())
    }
}
