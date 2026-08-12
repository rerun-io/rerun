// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A reference to a range of time.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct AbsoluteTimeRange {
    pub range: rerun::datatypes::AbsoluteTimeRange,
}
