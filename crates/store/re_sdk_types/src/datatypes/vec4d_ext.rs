use super::Vec4D;

impl Vec4D {
    /// The zero vector, i.e. the additive identity.
    pub const ZERO: Self = Self([0.0; 4]);

    /// Create a new vector.
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self([x, y, z, w])
    }

    /// The x-coordinate, i.e. index 0.
    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    /// The y-coordinate, i.e. index 1.
    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }

    /// The z-coordinate, i.e. index 2.
    #[inline]
    pub fn z(&self) -> f32 {
        self.0[2]
    }

    /// The w-coordinate, i.e. index 3.
    #[inline]
    pub fn w(&self) -> f32 {
        self.0[3]
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

impl<'a> From<&'a Self> for Vec4D {
    fn from(v: &'a Self) -> Self {
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

impl<Idx> std::ops::IndexMut<Idx> for Vec4D
where
    Idx: std::slice::SliceIndex<[f32]>,
{
    #[inline]
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output {
        &mut self.0[index]
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

#[cfg(feature = "mint")]
impl From<Vec4D> for mint::Vector4<f32> {
    #[inline]
    fn from(v: Vec4D) -> Self {
        Self {
            x: v[0],
            y: v[1],
            z: v[2],
            w: v[3],
        }
    }
}

#[cfg(feature = "mint")]
impl From<mint::Vector4<f32>> for Vec4D {
    #[inline]
    fn from(v: mint::Vector4<f32>) -> Self {
        Self([v.x, v.y, v.z, v.w])
    }
}

impl std::fmt::Display for Vec4D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prec = f.precision().unwrap_or(crate::DEFAULT_DISPLAY_DECIMALS);
        write!(
            f,
            "[{:.prec$}, {:.prec$}, {:.prec$}, {:.prec$}]",
            self.x(),
            self.y(),
            self.z(),
            self.w(),
        )
    }
}
