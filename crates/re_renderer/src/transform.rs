//! Transformation utilities.
//!
//! Some space definitions to keep in mind:
//!
//! Texture coordinates:
//! * origin top left
//! * full texture ([left; right], [top; bottom]):
//!     ([0; 1], [0; 1])
//!
//! NDC:
//! * origin center
//! * full screen ([left; right], [top; bottom]):
//!     ([-1; 1], [1; -1])
//!
//! Pixel coordinates:
//! * origin top left
//! * full screen ([left; right], [top; bottom]):
//!      ([0; `screen_extent.x`], [0; `screen_extent.y`])

use crate::rect::RectF32;

/// Transforms texture coordinates to normalized device coordinates (NDC).
#[inline]
pub fn ndc_from_texcoord(texcoord: glam::Vec2) -> glam::Vec2 {
    glam::vec2(texcoord.x * 2.0 - 1.0, 1.0 - texcoord.y * 2.0)
}

/// Transforms texture coordinates to normalized device coordinates (NDC).
#[inline]
pub fn ndc_from_pixel(pixel_coord: glam::Vec2, screen_extent: glam::UVec2) -> glam::Vec2 {
    glam::vec2(
        pixel_coord.x / screen_extent.x as f32 * 2.0 - 1.0,
        1.0 - pixel_coord.y / screen_extent.y as f32 * 2.0,
    )
}

#[derive(Clone, Debug)]
pub struct RectTransform {
    pub from: RectF32,
    pub to: RectF32,
}

impl RectTransform {
    /// No-op rect transform that transforms from a unit rectangle to a unit rectangle.
    pub const IDENTITY: RectTransform = RectTransform {
        from: RectF32::UNIT,
        to: RectF32::UNIT,
    };

    /// Computes a transformation matrix that applies the rect transform to the NDC space.
    ///
    ///
    /// Note only the relation of the rectangles in `RectTransform` is important.
    /// Scaling or moving both rectangles by the same amount does not change the result.
    pub fn to_ndc_scale_and_translation(&self) -> glam::Mat4 {
        // It's easier to think in texcoord space, and then transform to NDC.
        // This texcoord rect specifies the portion of the screen that should become the entire range of the NDC screen.
        let texcoord_rect = RectF32 {
            left_top: (self.from.left_top - self.to.left_top) / self.to.extent,
            extent: self.from.extent / self.to.extent,
        };
        let texcoord_rect_min = texcoord_rect.min();
        let texcoord_rect_max = texcoord_rect.max();

        // y axis is flipped in NDC, therefore we need to flip the y axis of the rect.
        let rect_min_ndc = ndc_from_texcoord(glam::vec2(texcoord_rect_min.x, texcoord_rect_max.y));
        let rect_max_ndc = ndc_from_texcoord(glam::vec2(texcoord_rect_max.x, texcoord_rect_min.y));

        let scale = 2.0 / (rect_max_ndc - rect_min_ndc);
        let translation = -0.5 * (rect_min_ndc + rect_max_ndc);

        glam::Mat4::from_scale(scale.extend(1.0))
            * glam::Mat4::from_translation(translation.extend(0.0))
    }

    pub fn scale(&self) -> glam::Vec2 {
        self.to.extent / self.from.extent
    }
}
