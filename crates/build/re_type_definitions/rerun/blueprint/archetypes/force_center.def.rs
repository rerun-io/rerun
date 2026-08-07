// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Tries to move the center of mass of the graph to the origin.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct ForceCenter {
    /// Whether the center force is enabled.
    ///
    /// The center force tries to move the center of mass of the graph towards the origin.
    #[rerun(optional)]
    pub enabled: Option<rerun::blueprint::components::Enabled>,

    /// The strength of the force.
    #[rerun(optional)]
    pub strength: Option<rerun::blueprint::components::ForceStrength>,
}
