// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A gamma correction value to be used with a scalar value or color.
///
/// Used to adjust the gamma of a color or scalar value between 0 and 1 before rendering.
/// `new_value = old_value ^ gamma`
///
/// Must be a positive number.
/// Defaults to 1.0 unless otherwise specified.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.ArrayLike")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct GammaCorrection {
    pub gamma: rerun::datatypes::Float32,
}
