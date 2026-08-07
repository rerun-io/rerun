// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Frames per second for a sequence timeline.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, PartialOrd))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct Fps {
    pub fps: rerun::datatypes::Float64,
}
