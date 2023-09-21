use super::UVec2D;

impl UVec2D {
    pub const ZERO: Self = Self([0; 2]);
    pub const ONE: Self = Self([1; 2]);

    #[inline]
    pub const fn new(x: u32, y: u32) -> Self {
        Self([x, y])
    }

    #[inline]
    pub fn x(&self) -> u32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> u32 {
        self.0[1]
    }
}

impl From<(u32, u32)> for UVec2D {
    #[inline]
    fn from((x, y): (u32, u32)) -> Self {
        Self::new(x, y)
    }
}

impl From<[u32; 2]> for UVec2D {
    #[inline]
    fn from(v: [u32; 2]) -> Self {
        Self(v)
    }
}

// NOTE: All these by-ref impls make the lives of end-users much easier when juggling around with
// slices, because Rust cannot keep track of the inherent `Copy` capability of it all across all the
// layers of `Into`/`IntoIterator`.

impl<'a> From<&'a UVec2D> for UVec2D {
    fn from(v: &'a UVec2D) -> Self {
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
