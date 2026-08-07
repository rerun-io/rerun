// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A force between each pair of nodes that ressembles an electrical charge.
///
/// If `strength` is smaller than 0, it pushes nodes apart, if it is larger than 0 it pulls them together.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct ForceManyBody {
    /// Whether the many body force is enabled.
    ///
    /// The many body force is applied on each pair of nodes in a way that ressembles an electrical charge. If the
    /// strength is smaller than 0, it pushes nodes apart; if it is larger than 0, it pulls them together.
    #[rerun(optional)]
    pub enabled: Option<rerun::blueprint::components::Enabled>,

    /// The strength of the force.
    ///
    /// If `strength` is smaller than 0, it pushes nodes apart, if it is larger than 0 it pulls them together.
    #[rerun(optional)]
    pub strength: Option<rerun::blueprint::components::ForceStrength>,
}
