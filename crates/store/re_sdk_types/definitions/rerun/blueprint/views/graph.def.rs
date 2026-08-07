// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A graph view to display time-variying, directed or undirected graph visualization.
///
/// \example views/graph title="Use a blueprint to create a graph view." image="https://static.rerun.io/graph_lattice/f9169da9c3f35b7260c9d74cd5be5fe710aec6a8/1200w.png"
#[rerun::rerun_type]
#[rerun(view_identifier = "Graph")]
#[rerun(state = "unstable")]
pub struct GraphView {
    /// Configures the background of the graph.
    pub background: rerun::blueprint::archetypes::GraphBackground,

    /// Everything within these bounds is guaranteed to be visible.
    ///
    /// Some things outside of these bounds may also be visible due to letterboxing.
    pub visual_bounds: rerun::blueprint::archetypes::VisualBounds2D,

    /// Allows to control the interaction between two nodes connected by an edge.
    pub force_link: rerun::blueprint::archetypes::ForceLink,

    /// A force between each pair of nodes that ressembles an electrical charge.
    pub force_many_body: rerun::blueprint::archetypes::ForceManyBody,

    /// Similar to gravity, this force pulls nodes towards a specific position.
    pub force_position: rerun::blueprint::archetypes::ForcePosition,

    /// Resolves collisions between the bounding spheres, according to the radius of the nodes.
    pub force_collision_radius: rerun::blueprint::archetypes::ForceCollisionRadius,

    /// Tries to move the center of mass of the graph to the origin.
    pub force_center: rerun::blueprint::archetypes::ForceCenter,
}
