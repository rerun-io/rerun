use super::UVec2D;

impl UVec2D {
    /// The zero vector, i.e. the additive identity.
    pub const ZERO: Self = Self([0; 2]);

    /// The unit vector `[1, 1]`, i.e. the multiplicative identity.
    pub const ONE: Self = Self([1; 2]);

    /// Create a new vector.
    #[inline]
    pub const fn new(x: u32, y: u32) -> Self {
        Self([x, y])
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

    /// Assign a new x
    #[inline]
    pub fn set_x(&mut self, x: u32) {
        self.0[0] = x;
    }

    /// Assign a new y
    #[inline]
    pub fn set_y(&mut self, y: u32) {
        self.0[1] = y;
    }
}

impl From<(u32, u32)> for UVec2D {
    #[inline]
    fn from((x, y): (u32, u32)) -> Self {
        Self::new(x, y)
    }
}

// NOTE: All these by-ref impls make the lives of end-users much easier when juggling around with
// slices, because Rust cannot keep track of the inherent `Copy` capability of it all across all the
// layers of `Into`/`IntoIterator`.

impl<'a> From<&'a Self> for UVec2D {
    fn from(v: &'a Self) -> Self {
        Self(v.0)
    }
}

impl<'a> From<&'a (u32, u32)> for UVec2D {
    #[inline]
    fn from((x, y): &'a (u32, u32)) -> Self {
        Self::new(*x, *y)
    }
}

impl<'a> From<&'a [u32; 2]> for UVec2D {
    #[inline]
    fn from(v: &'a [u32; 2]) -> Self {
        Self(*v)
    }
}

impl<Idx> std::ops::Index<Idx> for UVec2D
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
impl From<UVec2D> for glam::UVec2 {
    fn from(v: UVec2D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::UVec2> for UVec2D {
    fn from(v: glam::UVec2) -> Self {
        Self(v.to_array())
    }
}

impl std::fmt::Display for UVec2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}]", self.x(), self.y())
    }
}
