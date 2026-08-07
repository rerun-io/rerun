// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Defines how points are shaded.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
pub enum PointShading {
    /// Radial gradient for a spherical shadow effect.
    #[default]
    Gradient = 1,

    /// Flat shading.
    Flat = 2,
}
