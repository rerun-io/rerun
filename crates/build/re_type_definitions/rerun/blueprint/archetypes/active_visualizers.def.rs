// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Override the visualizers for an entity.
///
/// This archetype is a stop-gap mechanism based on the current implementation details
/// of the visualizer system. It is not intended to be a long-term solution, but provides
/// enough utility to be useful in the short term.
///
/// This can only be used as part of blueprints. It will have no effect if used
/// in a regular entity.
#[rerun::rerun_type]
#[python(aliases = "str | Sequence[str]")]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct ActiveVisualizers {
    /// Id's of the visualizers that should be active.
    #[rerun(required)]
    pub instruction_ids: Vec<rerun::blueprint::components::VisualizerInstructionId>,
}
