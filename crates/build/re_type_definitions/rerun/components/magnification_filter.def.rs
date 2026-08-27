// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Filter used when a single texel/pixel of an image is displayed larger than a single screen pixel.
///
/// This happens when zooming into an image, when displaying a low-resolution image in a large area,
/// or when viewing an image up close in 3D space.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
pub enum MagnificationFilter {
    /// Show the nearest pixel value.
    ///
    /// This will give a blocky appearance when the image is scaled up.
    /// Used as default when rendering 2D images.
    #[default]
    Nearest = 1,

    /// Linearly interpolate the nearest neighbors, creating a smoother look when the image is scaled up.
    ///
    /// Used as default for mesh rendering.
    Linear = 2,

    /// Bicubic interpolation using a Catmull-Rom spline, creating the smoothest look when the image is scaled up.
    ///
    /// This is computationally more expensive than linear filtering but produces sharper results with less blurring.
    /// Unlike bilinear filtering, this avoids cross-shaped artifacts at texel boundaries.
    Bicubic = 3,
}
