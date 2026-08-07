// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// ID for a visualizer instruction.
///
/// IDs are only guaranteed to be unique in the scope of a view.
/// For details see [archetypes.ActiveVisualizers].
#[rerun::rerun_type]
#[python(aliases = "str | list[str]")]
#[python(array_aliases = "str")]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, PartialOrd, Ord, Default, Hash, Copy))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct VisualizerInstructionId {
    /// IDs of a single visualizer instruction.
    pub visualizer: rerun::datatypes::Uuid,
}
