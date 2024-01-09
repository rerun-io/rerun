use super::Vec4D;

impl Vec4D {
    pub const ZERO: Vec4D = Vec4D([0.0; 4]);

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self([x, y, z, w])
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }

    #[inline]
    pub fn z(&self) -> f32 {
        self.0[2]
    }

    #[inline]
    pub fn w(&self) -> f32 {
        self.0[2]
    }
}

impl From<(f32, f32, f32, f32)> for Vec4D {
    #[inline]
    fn from((x, y, z, w): (f32, f32, f32, f32)) -> Self {
        Self::new(x, y, z, w)
    }
}

// NOTE: All these by-ref impls make the lives of end-users much easier when juggling around with
// slices, because Rust cannot keep track of the inherent `Copy` capability of it all across all the
// layers of `Into`/`IntoIterator`.

impl<'a> From<&'a Vec4D> for Vec4D {
    fn from(v: &'a Vec4D) -> Self {
        Self(v.0)
    }
}

impl<'a> From<&'a (f32, f32, f32, f32)> for Vec4D {
    #[inline]
    fn from((x, y, z, w): &'a (f32, f32, f32, f32)) -> Self {
        Self::new(*x, *y, *z, *w)
    }
}

impl<'a> From<&'a [f32; 4]> for Vec4D {
    #[inline]
    fn from(v: &'a [f32; 4]) -> Self {
        Self(*v)
    }
}

impl<Idx> std::ops::Index<Idx> for Vec4D
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

#[cfg(feature = "glam")]
impl From<Vec4D> for glam::Vec4 {
    #[inline]
    fn from(v: Vec4D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec4> for Vec4D {
    #[inline]
    fn from(v: glam::Vec4) -> Self {
        Self(v.to_array())
    }
}

impl std::fmt::Display for Vec4D {
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
