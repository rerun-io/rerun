// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Similar to gravity, this force pulls nodes towards a specific position.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct ForcePosition {
    /// Whether the position force is enabled.
    ///
    /// The position force pulls nodes towards a specific position, similar to gravity.
    #[rerun(optional)]
    pub enabled: Option<rerun::blueprint::components::Enabled>,

    /// The strength of the force.
    #[rerun(optional)]
    pub strength: Option<rerun::blueprint::components::ForceStrength>,

    /// The position where the nodes should be pulled towards.
    #[rerun(optional)]
    pub position: Option<rerun::components::Position2D>,
}
