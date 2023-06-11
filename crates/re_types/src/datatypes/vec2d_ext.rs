use super::Vec2D;

impl Vec2D {
    pub const ZERO: Self = Self::new(0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self([x, y])
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.0[0]
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.0[1]
    }
}

impl From<(f32, f32)> for Vec2D {
    #[inline]
    fn from((x, y): (f32, f32)) -> Self {
        Self::new(x, y)
    }
}

impl From<[f32; 2]> for Vec2D {
    #[inline]
    fn from(v: [f32; 2]) -> Self {
        Self(v)
    }
}

impl<Idx> std::ops::Index<Idx> for Vec2D
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
impl From<Vec2D> for glam::Vec2 {
    fn from(v: Vec2D) -> Self {
        Self::from_slice(&v.0)
    }
}

#[cfg(feature = "glam")]
impl From<glam::Vec2> for Vec2D {
    fn from(v: glam::Vec2) -> Self {
        Self(v.to_array())
    }
}

impl std::fmt::Display for Vec2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:.prec$}, {:.prec$}]",
            self.x(),
            self.y(),
            prec = crate::DISPLAY_PRECISION,
        )
    }
}
