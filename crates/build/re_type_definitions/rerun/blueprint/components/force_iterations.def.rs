// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Specifies how often this force should be applied per iteration.
///
/// Increasing this parameter can lead to better results at the cost of longer computation time.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, Copy, PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct ForceIterations {
    pub distance: rerun::datatypes::UInt64,
}
