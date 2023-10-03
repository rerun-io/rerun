use super::UVec4D;

impl UVec4D {
    pub const ZERO: Self = Self([0; 4]);
    pub const ONE: Self = Self([1; 4]);

    #[inline]
    pub const fn new(x: u32, y: u32, z: u32, w: u32) -> Self {
        Self([x, y, z, w])
    }

    #[inline]
    pub fn x(&self) -> u32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> u32 {
        self.0[1]
    }

    #[inline]
    pub fn z(&self) -> u32 {
        self.0[2]
    }

    #[inline]
    pub fn w(&self) -> u32 {
        self.0[2]
    }
}

impl From<(u32, u32, u32, u32)> for UVec4D {
    #[inline]
    fn from((x, y, z, w): (u32, u32, u32, u32)) -> Self {
        Self::new(x, y, z, w)
    }
}

// NOTE: All these by-ref impls make the lives of end-users much easier when juggling around with
// slices, because Rust cannot keep track of the inherent `Copy` capability of it all across all the
// layers of `Into`/`IntoIterator`.

impl<'a> From<&'a UVec4D> for UVec4D {
    fn from(v: &'a UVec4D) -> Self {
        Self(v.0)
    }
}

impl<'a> From<&'a (u32, u32, u32, u32)> for UVec4D {
    #[inline]
    fn from((x, y, z, w): &'a (u32, u32, u32, u32)) -> Self {
        Self::new(*x, *y, *z, *w)
    }
}

impl<'a> From<&'a [u32; 4]> for UVec4D {
    #[inline]
    fn from(v: &'a [u32; 4]) -> Self {
        Self(*v)
    }
}

impl<Idx> std::ops::Index<Idx> for UVec4D
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
impl From<UVec4D> for glam::UVec3 {
    #[inline]
    fn from(v: UVec4D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::UVec4> for UVec4D {
    #[inline]
    fn from(v: glam::UVec4) -> Self {
        Self(v.to_array())
    }
}

impl std::fmt::Display for UVec4D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:.prec$}, {:.prec$}, {:.prec$}, {:.prec$}]",
            self.x(),
            self.y(),
            self.z(),
            self.w(),
            prec = crate::DISPLAY_PRECISION,
        )
    }
}
