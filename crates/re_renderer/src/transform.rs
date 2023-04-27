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

/// Computes a transformation matrix that transforms normalized device coordinates
/// to a region of interest.
///
/// Note that this be used for zooming out/in & panning in NDC space.
/// For ease of use, the region of interest that defines the entire screen is defined in a texture coordinate
/// space. Meaning that the identity transform is defined by the unit rectangle, `RectF32::UNIT`.
pub fn region_of_interest_from_ndc(texcoord_region_of_interest: RectF32) -> glam::Mat4 {
    let rect_min = texcoord_region_of_interest.min();
    let rect_max = texcoord_region_of_interest.max();

    // y axis is flipped in NDC, therefore we need to flip the y axis of the rect.
    let rect_min_ndc = ndc_from_texcoord(glam::vec2(rect_min.x, rect_max.y));
    let rect_max_ndc = ndc_from_texcoord(glam::vec2(rect_max.x, rect_min.y));

    let scale = 2.0 / (rect_max_ndc - rect_min_ndc);
    let translation = -0.5 * (rect_min_ndc + rect_max_ndc);

    glam::Mat4::from_scale(scale.extend(1.0))
        * glam::Mat4::from_translation(translation.extend(0.0))
}
