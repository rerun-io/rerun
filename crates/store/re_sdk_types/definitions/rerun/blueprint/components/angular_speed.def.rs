// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Angular speed, used for rotation speed for example.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct AngularSpeed {
    /// Speed value in radians per second.
    pub speed: rerun::datatypes::Float64,
}
