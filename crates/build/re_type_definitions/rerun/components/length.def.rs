// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Length, or one-dimensional size.
///
/// Measured in its local coordinate system; consult the archetype in use to determine which
/// axis or part of the entity this is the length of.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.NDArray[np.float32]")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct Length {
    pub length: rerun::datatypes::Float32,
}
