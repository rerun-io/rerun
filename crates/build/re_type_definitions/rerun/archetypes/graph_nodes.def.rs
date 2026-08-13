// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A list of nodes in a graph with optional labels, colors, etc.
///
/// \example archetypes/graph_undirected !api title="Simple undirected graph" image="https://static.rerun.io/graph_undirected/15f46bec77452a8c6220558e4403b99cac188e2e/1200w.png"
/// \example archetypes/graph_directed title="Simple directed graph" image="https://static.rerun.io/graph_directed/ca29a37b65e1e0b6482251dce401982a0bc568fa/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Graph")]
#[docs(view_types = "GraphView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "GraphNodes")]
#[rust(derive(PartialEq))]
pub struct GraphNodes {
    /// A list of node IDs.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub node_ids: Vec<rerun::components::GraphNode>,

    /// Optional center positions of the nodes.
    #[rerun(optional)]
    pub positions: Option<Vec<rerun::components::Position2D>>,

    /// Optional colors for the boxes.
    #[rerun(optional)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Optional text labels for the node.
    #[rerun(optional)]
    pub labels: Option<Vec<rerun::components::Text>>,

    /// Whether the text labels should be shown.
    ///
    /// If not set, labels will automatically appear when there is exactly one label for this entity
    /// or the number of instances on this entity is under a certain threshold.
    #[rerun(optional)]
    pub show_labels: Option<rerun::components::ShowLabels>,

    /// Optional radii for nodes.
    #[rerun(optional)]
    pub radii: Option<Vec<rerun::components::Radius>>,
}
