// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Degree of transparency ranging from 0.0 (fully transparent) to 1.0 (fully opaque).
///
/// The final opacity value may be a result of multiplication with alpha values as specified by other color sources.
/// Unless otherwise specified, the default value is 1.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.ArrayLike")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct Opacity {
    pub opacity: rerun::datatypes::Float32,
}
