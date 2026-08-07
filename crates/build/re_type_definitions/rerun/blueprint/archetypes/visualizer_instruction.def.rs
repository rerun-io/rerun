// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A visualizer instruction for an entity.
#[rerun::rerun_type]
#[python(aliases = "str | Sequence[str]")]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct VisualizerInstruction {
    /// The type of the visualizer.
    #[rerun(required)]
    pub visualizer_type: rerun::blueprint::components::VisualizerType,

    /// The component mapping pairs.
    #[rerun(optional)]
    pub component_map: Option<Vec<rerun::blueprint::components::VisualizerComponentMapping>>,
}
