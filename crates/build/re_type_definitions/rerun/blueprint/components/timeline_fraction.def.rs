// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A position on a timeline, as a fraction of its time range.
///
/// 0.0 is the start and 1.0 the end.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, PartialOrd))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct TimelineFraction {
    pub fraction: rerun::encodings::Float64,
}
