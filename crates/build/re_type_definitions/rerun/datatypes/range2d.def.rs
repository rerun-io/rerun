// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An Axis-Aligned Bounding Box in 2D space, implemented as the minimum and maximum corners.
#[rerun::rerun_type]
#[rust(derive(Default, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "C")]
#[rerun(state = "stable")]
pub struct Range2D {
    /// The range of the X-axis (usually left and right bounds).
    pub x_range: rerun::datatypes::Range1D,

    /// The range of the Y-axis (usually top and bottom bounds).
    pub y_range: rerun::datatypes::Range1D,
}
