use super::DVec2D;

impl DVec2D {
    /// The zero vector, i.e. the additive identity.
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// The unit vector `[1, 1]`, i.e. the multiplicative identity.
    pub const ONE: Self = Self::new(1.0, 1.0);

    /// Create a new vector.
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self([x, y])
    }

    /// The x-coordinate, i.e. index 0.
    #[inline]
    pub fn x(&self) -> f64 {
        self.0[0]
    }

    /// The y-coordinate, i.e. index 1.
    #[inline]
    pub fn y(&self) -> f64 {
        self.0[1]
    }
}

impl From<(f64, f64)> for DVec2D {
    #[inline]
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

// NOTE: All these by-ref impls make the lives of end-users much easier when juggling around with
// slices, because Rust cannot keep track of the inherent `Copy` capability of it all across all the
// layers of `Into`/`IntoIterator`.

impl<'a> From<&'a Self> for DVec2D {
    fn from(v: &'a Self) -> Self {
        Self(v.0)
    }
}

impl<'a> From<&'a (f64, f64)> for DVec2D {
    #[inline]
    fn from((x, y): &'a (f64, f64)) -> Self {
        Self::new(*x, *y)
    }
}

impl<'a> From<&'a [f64; 2]> for DVec2D {
    #[inline]
    fn from(v: &'a [f64; 2]) -> Self {
        Self(*v)
    }
}

impl<Idx> std::ops::Index<Idx> for DVec2D
where
    Idx: std::slice::SliceIndex<[f64]>,
{
    type Output = Idx::Output;

    #[inline]
    fn index(&self, index: Idx) -> &Self::Output {
        &self.0[index]
    }
}

impl<Idx> std::ops::IndexMut<Idx> for DVec2D
where
    Idx: std::slice::SliceIndex<[f64]>,
{
    #[inline]
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output {
        &mut self.0[index]
    }
}

#[cfg(feature = "mint")]
impl From<DVec2D> for mint::Vector2<f64> {
    #[inline]
    fn from(v: DVec2D) -> Self {
        Self { x: v[0], y: v[1] }
    }
}

#[cfg(feature = "mint")]
impl From<mint::Vector2<f64>> for DVec2D {
    #[inline]
    fn from(v: mint::Vector2<f64>) -> Self {
        Self([v.x, v.y])
    }
}

impl std::fmt::Display for DVec2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prec = f.precision().unwrap_or(crate::DEFAULT_DISPLAY_DECIMALS);
        write!(f, "[{:.prec$}, {:.prec$}]", self.x(), self.y(),)
    }
}
