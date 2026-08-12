// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Indicate whether the range should be locked when zooming in on the data.
///
/// Default is `false`, i.e. zoom will change the visualized range.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct LockRangeDuringZoom {
    pub lock_range: rerun::datatypes::Bool,
}
