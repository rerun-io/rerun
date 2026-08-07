// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A list of edges in a graph.
///
/// By default, edges are undirected.
///
/// \example archetypes/graph_undirected !api title="Simple undirected graph" image="https://static.rerun.io/graph_undirected/15f46bec77452a8c6220558e4403b99cac188e2e/1200w.png"
/// \example archetypes/graph_directed title="Simple directed graph" image="https://static.rerun.io/graph_directed/ca29a37b65e1e0b6482251dce401982a0bc568fa/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Graph")]
#[docs(view_types = "GraphView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "GraphEdges")]
#[rust(derive(PartialEq))]
pub struct GraphEdges {
    /// A list of node tuples.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub edges: Vec<rerun::components::GraphEdge>,

    /// Specifies if the graph is directed or undirected.
    ///
    /// If no [components.GraphType] is provided, the graph is assumed to be undirected.
    #[rerun(recommended)]
    pub graph_type: Option<rerun::components::GraphType>,
}
