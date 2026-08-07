// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A time range on an unspecified timeline using either relative or absolute boundaries.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct TimeRange {
    pub time_range: rerun::datatypes::TimeRange,
}
