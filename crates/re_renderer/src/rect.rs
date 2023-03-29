/// A 2D rectangle with integer coordinates.
///
/// Typically used for texture cutouts etc.
#[derive(Clone, Copy, Debug)]
pub struct IntRect {
    /// The top left corner of the rectangle.
    pub top_left_corner: glam::IVec2,

    /// The size of the rectangle.
    pub extent: glam::UVec2,
}

impl IntRect {
    #[inline]
    pub fn from_middle_and_extent(middle: glam::IVec2, size: glam::UVec2) -> Self {
        Self {
            top_left_corner: middle - size.as_ivec2() / 2,
            extent: size,
        }
    }

    #[inline]
    pub fn width(&self) -> u32 {
        self.extent.x
    }

    #[inline]
    pub fn height(&self) -> u32 {
        self.extent.x
    }

    #[inline]
    pub fn wgpu_extent(&self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.extent.x,
            height: self.extent.y,
            depth_or_array_layers: 1,
        }
    }
}
