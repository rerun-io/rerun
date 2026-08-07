// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A connection between two [datatypes.KeypointId]s.
#[rerun::rerun_type]
#[python(aliases = "Sequence[datatypes.KeypointIdLike]")]
#[rust(derive(Default, Eq, PartialEq))]
#[rerun(state = "stable")]
pub struct KeypointPair {
    /// The first point of the pair.
    pub keypoint0: rerun::datatypes::KeypointId,

    /// The second point of the pair.
    pub keypoint1: rerun::datatypes::KeypointId,
}
