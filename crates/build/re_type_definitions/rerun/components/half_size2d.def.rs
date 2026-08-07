// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Half-size (radius) of a 2D box.
///
/// Measured in its local coordinate system.
///
/// The box extends both in negative and positive direction along each axis.
/// Negative sizes indicate that the box is flipped along the respective axis, but this has no effect on how it is displayed.
#[rerun::rerun_type]
#[rust(derive(Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct HalfSize2D {
    pub xy: rerun::datatypes::Vec2D,
}
