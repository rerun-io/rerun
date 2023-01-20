/// A size of something in either world-units, screen-units, or unsized.
///
/// Implementation:
/// * If positive, this is in scene units.
/// * If negative, this is in points.
/// * If NaN, auto-size it.
///
/// Resolved on-the-fly in shader code. See shader/utils/size.wgsl
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Size(pub f32);

impl Size {
    /// Automatically sized, based on a view builder setting.
    pub const AUTO: Self = Self(f32::INFINITY);

    /// Like [`Size::AUTO`], but larger by some small factor (~2).
    pub const AUTO_LARGE: Self = Self(-f32::INFINITY);

    /// Creates a new size in scene units.
    ///
    /// Values passed must be finite positive.
    #[inline]
    pub fn new_scene(size: f32) -> Self {
        debug_assert!(size.is_finite() && size >= 0.0, "Bad size: {size}");
        Self(size)
    }

    /// Creates a new size in ui point units.
    ///
    /// Values passed must be finite positive.
    #[inline]
    pub fn new_points(size: f32) -> Self {
        debug_assert!(size.is_finite() && size >= 0.0, "Bad size: {size}");
        Self(-size)
    }

    /// Returns true if the size is an automatically determined size ([`Self::AUTO`] or [`Self::AUTO_LARGE`]).
    #[inline]
    pub fn is_auto(&self) -> bool {
        self.0.is_infinite()
    }

    /// Get the scene-size of this, if stored as a scene size.
    #[inline]
    #[allow(unused)] // wgpu is not yet using this
    pub fn scene(&self) -> Option<f32> {
        (self.0.is_finite() && self.0 >= 0.0).then_some(self.0)
    }

    /// Get the point size of this, if stored as a point size.
    #[inline]
    pub fn points(&self) -> Option<f32> {
        (self.0.is_finite() && self.0 <= 0.0).then_some(-self.0)
    }
}

impl PartialEq for Size {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.is_nan() && other.0.is_nan() || self.0 == other.0
    }
}

impl std::ops::Mul<f32> for Size {
    type Output = Size;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        debug_assert!(rhs.is_finite() && rhs >= 0.0);
        Self(self.0 * rhs)
    }
}

impl std::ops::MulAssign<f32> for Size {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        debug_assert!(rhs.is_finite() && rhs >= 0.0);
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
        SizeHalf(half::f16::from_f32(size.0))
    }
}
