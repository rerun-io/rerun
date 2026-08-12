// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Radius of a marker of a point in e.g. a 2D plot, measured in UI points.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.ArrayLike")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct MarkerSize {
    pub value: rerun::datatypes::Float32,
}
