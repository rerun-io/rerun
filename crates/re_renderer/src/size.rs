/// A size of something in either scene units or ui points.
///
/// Implementation:
/// * If positive, this is in scene units.
/// * If negative, this is in ui points.
///
/// Resolved on-the-fly in shader code. See shader/utils/size.wgsl
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Size(pub f32);

impl Size {
    /// Zero radius.
    pub const ZERO: Self = Self(0.0);

    /// Radius of length 1 in scene units.
    pub const ONE: Self = Self(1.0);

    /// Radius of length 1 in ui points.
    pub const ONE_UI_POINTS: Self = Self(-1.0);

    /// Creates a new size in scene units.
    ///
    /// Values passed must be finite positive.
    #[inline]
    pub fn new_scene_units(size: f32) -> Self {
        debug_assert!((0.0..=f32::INFINITY).contains(&size), "Bad size: {size}");
        Self(size)
    }

    /// Creates a new size in ui point units.
    ///
    /// Values passed must be finite positive.
    #[inline]
    pub fn new_ui_points(size: f32) -> Self {
        debug_assert!((0.0..=f32::INFINITY).contains(&size), "Bad size: {size}");
        Self(-size)
    }

    /// Get the scene-size of this, if stored as a scene size.
    #[inline]
    pub fn scene(&self) -> Option<f32> {
        (self.0 >= 0.0).then_some(self.0)
    }

    /// Get the point size of this, if stored as a point size.
    #[inline]
    pub fn points(&self) -> Option<f32> {
        (self.0 < 0.0).then_some(-self.0)
    }
}

impl PartialEq for Size {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.is_nan() && other.0.is_nan() || self.0 == other.0
    }
}

impl std::ops::Mul<f32> for Size {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        debug_assert!(rhs.is_finite() && rhs >= 0.0);
        debug_assert!((self.0 * rhs).is_finite());
        Self(self.0 * rhs)
    }
}

impl std::ops::MulAssign<f32> for Size {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        debug_assert!(rhs.is_finite() && rhs >= 0.0);
        debug_assert!((self.0 * rhs).is_finite());
        self.0 *= rhs;
    }
}

/// Same as [`Size`] but stored with a f16 float.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SizeHalf(half::f16);

impl From<Size> for SizeHalf {
    #[inline]
    fn from(size: Size) -> Self {
        Self(half::f16::from_f32(size.0))
    }
}
