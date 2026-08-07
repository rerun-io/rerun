// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for the filter-by-range feature of the dataframe view.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq))]
#[rerun(state = "unstable")]
pub struct FilterByRange {
    /// Beginning of the time range.
    pub start: rerun::datatypes::TimeInt,

    /// End of the time range (inclusive).
    pub end: rerun::datatypes::TimeInt,
}
