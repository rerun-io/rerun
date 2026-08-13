// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Resolves collisions between the bounding circles, according to the radius of the nodes.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct ForceCollisionRadius {
    /// Whether the collision force is enabled.
    ///
    /// The collision force resolves collisions between nodes based on the bounding circle defined by their radius.
    #[rerun(optional)]
    pub enabled: Option<rerun::blueprint::components::Enabled>,

    /// The strength of the force.
    #[rerun(optional)]
    pub strength: Option<rerun::blueprint::components::ForceStrength>,

    /// Specifies how often this force should be applied per iteration.
    ///
    /// Increasing this parameter can lead to better results at the cost of longer computation time.
    #[rerun(optional)]
    pub iterations: Option<rerun::blueprint::components::ForceIterations>,
}
