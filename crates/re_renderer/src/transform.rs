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

/// Defines a transformation from a rectangular region of interest into a rectangular target region.
///
/// Transforms map the range of `region_of_interest` to the range of `region`.
#[derive(Clone, Debug)]
pub struct RectTransform {
    pub region_of_interest: RectF32,
    pub region: RectF32,
}

impl RectTransform {
    /// No-op rect transform that transforms from a unit rectangle to a unit rectangle.
    pub const IDENTITY: RectTransform = RectTransform {
        region_of_interest: RectF32::UNIT,
        region: RectF32::UNIT,
    };

    /// Computes a transformation matrix that applies the rect transform to the NDC space.
    ///
    /// This matrix is expected to be the left most transformation in the vertex transformation chain.
    /// It causes the area described by `region_of_interest` to be mapped to the area described by `region`.
    /// Meaning, that `region` represents the full screen of the NDC space.
    ///
    /// This means that only the relation of the rectangles in `RectTransform` is important.
    /// Scaling or moving both rectangles by the same amount does not change the result.
    pub fn to_ndc_scale_and_translation(&self) -> glam::Mat4 {
        // It's easier to think in texcoord space, and then transform to NDC.
        // This texcoord rect specifies the portion of the screen that should become the entire range of the NDC screen.
        let texcoord_rect = RectF32 {
            min: (self.region_of_interest.min - self.region.min) / self.region.extent,
            extent: self.region_of_interest.extent / self.region.extent,
        };
        let texcoord_rect_min = texcoord_rect.min;
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
        self.region.extent / self.region_of_interest.extent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn to_ndc_scale_and_translation() {
        let region = RectF32 {
            min: glam::vec2(1.0, 1.0),
            extent: glam::vec2(2.0, 3.0),
        };

        // Identity
        {
            let rect_transform = RectTransform {
                region_of_interest: region,
                region,
            };
            let identity = rect_transform.to_ndc_scale_and_translation();
            assert_eq!(identity, glam::Mat4::IDENTITY);
        }

        // Scale
        {
            let scale_factor = glam::vec2(2.0, 0.25);

            let rect_transform = RectTransform {
                region_of_interest: RectF32 {
                    // Move the roi to the middle of the region.
                    min: region.center() - region.extent * scale_factor * 0.5,
                    extent: region.extent * scale_factor,
                },
                region,
            };
            let scale = rect_transform.to_ndc_scale_and_translation();
            assert_eq!(
                scale,
                glam::Mat4::from_scale(1.0 / scale_factor.extend(1.0))
            );
        }

        // Translation
        {
            let translation_vec = glam::vec2(1.0, 2.0);

            let rect_transform = RectTransform {
                region_of_interest: RectF32 {
                    min: region.min + translation_vec * region.extent,
                    extent: region.extent,
                },
                region,
            };
            let translation = rect_transform.to_ndc_scale_and_translation();
            assert_eq!(
                translation,
                glam::Mat4::from_translation(
                    glam::vec3(-translation_vec.x, translation_vec.y, 0.0) * 2.0
                )
            );
        }

        // Scale + translation
        {
            let scale_factor = glam::vec2(2.0, 0.25);
            let translation_vec = glam::vec2(1.0, 2.0);

            let rect_transform = RectTransform {
                region_of_interest: RectF32 {
                    // Move the roi to the middle of the region and then apply translation
                    min: region.center() - region.extent * scale_factor * 0.5
                        + translation_vec * region.extent,
                    extent: region.extent * scale_factor,
                },
                region,
            };
            let scale_and_translation = rect_transform.to_ndc_scale_and_translation();
            assert_eq!(
                scale_and_translation,
                glam::Mat4::from_scale(1.0 / scale_factor.extend(1.0))
                    * glam::Mat4::from_translation(
                        glam::vec3(-translation_vec.x, translation_vec.y, 0.0) * 2.0
                    )
            );
        }
    }
}
