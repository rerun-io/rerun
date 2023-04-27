/// A 2D rectangle with integer coordinates.
///
/// Typically used for texture cutouts etc.
#[derive(Clone, Copy, Debug)]
pub struct RectInt {
    /// The top left corner of the rectangle.
    pub left_top: glam::IVec2,

    /// The size of the rectangle.
    pub extent: glam::UVec2,
}

impl RectInt {
    #[inline]
    pub fn from_middle_and_extent(middle: glam::IVec2, size: glam::UVec2) -> Self {
        Self {
            left_top: middle - size.as_ivec2() / 2,
            extent: size,
        }
    }

    #[inline]
    pub fn width(self) -> u32 {
        self.extent.x
    }

    #[inline]
    pub fn height(self) -> u32 {
        self.extent.y
    }

    #[inline]
    pub fn wgpu_extent(self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.extent.x,
            height: self.extent.y,
            depth_or_array_layers: 1,
        }
    }
}

/// A 2D rectangle with float coordinates.
#[derive(Clone, Copy, Debug)]
pub struct RectF32 {
    /// The top left corner of the rectangle.
    pub left_top: glam::Vec2,

    /// The size of the rectangle. Supposed to be positive.
    pub extent: glam::Vec2,
}

impl RectF32 {
    /// The unit rectangle, defined as (0, 0) - (1, 1).
    pub const UNIT: RectF32 = RectF32 {
        left_top: glam::Vec2::ZERO,
        extent: glam::Vec2::ONE,
    };

    #[inline]
    pub fn min(self) -> glam::Vec2 {
        self.left_top
    }

    #[inline]
    pub fn max(self) -> glam::Vec2 {
        self.left_top + self.extent
    }

    #[inline]
    pub fn scale_extent(self, factor: f32) -> RectF32 {
        RectF32 {
            left_top: self.left_top * factor,
            extent: self.extent * factor,
        }
    }
}

impl From<RectInt> for RectF32 {
    #[inline]
    fn from(rect: RectInt) -> Self {
        Self {
            left_top: rect.left_top.as_vec2(),
            extent: rect.extent.as_vec2(),
        }
    }
}
