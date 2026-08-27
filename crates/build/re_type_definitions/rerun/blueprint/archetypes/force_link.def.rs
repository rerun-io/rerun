// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Aims to achieve a target distance between two nodes that are connected by an edge.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct ForceLink {
    /// Whether the link force is enabled.
    ///
    /// The link force aims to achieve a target distance between two nodes that are connected by one ore more edges.
    #[rerun(optional)]
    pub enabled: Option<rerun::blueprint::components::Enabled>,

    /// The target distance between two nodes.
    #[rerun(optional)]
    pub distance: Option<rerun::blueprint::components::ForceDistance>,

    /// Specifies how often this force should be applied per iteration.
    ///
    /// Increasing this parameter can lead to better results at the cost of longer computation time.
    #[rerun(optional)]
    pub iterations: Option<rerun::blueprint::components::ForceIterations>,
}
