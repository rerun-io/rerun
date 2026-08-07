// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The type of the visualizer.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord, Default))]
#[rerun(state = "unstable")]
pub struct VisualizerType {
    /// The type of the visualizer.
    pub visualizer_type: rerun::datatypes::Utf8,
}
