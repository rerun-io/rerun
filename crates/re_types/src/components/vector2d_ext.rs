use super::Vector2D;

impl Vector2D {
    pub const ZERO: Self = Self(crate::datatypes::Vec2D::ZERO);
    pub const ONE: Self = Self(crate::datatypes::Vec2D::ONE);
}

#[cfg(feature = "glam")]
impl From<Vector2D> for glam::Vec2 {
    #[inline]
    fn from(v: Vector2D) -> Self {
        Self::new(v.x(), v.y())
    }
}

#[cfg(feature = "mint")]
impl From<Vector2D> for mint::Vector2<f32> {
    #[inline]
    fn from(v: Vector2D) -> Self {
        Self { x: v.x(), y: v.y() }
    }
}
