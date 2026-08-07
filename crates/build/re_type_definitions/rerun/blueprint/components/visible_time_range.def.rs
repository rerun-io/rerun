// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The range of values on a given timeline that will be included in a view's query.
///
/// Refer to `VisibleTimeRanges` archetype for more information.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct VisibleTimeRange {
    pub value: rerun::datatypes::VisibleTimeRange,
}
