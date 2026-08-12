// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Associates components of an entity to components of a visualizer.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq))]
#[rerun(state = "unstable")]
pub struct VisualizerComponentMapping {
    /// The component mapping pairs.
    pub mapping: rerun::blueprint::datatypes::VisualizerComponentMapping,
}
