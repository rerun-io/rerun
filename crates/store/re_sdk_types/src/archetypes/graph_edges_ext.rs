use super::GraphEdges;

impl GraphEdges {
    /// Creates a graph with undirected edges.
    #[inline(always)]
    pub fn with_undirected_edges(self) -> Self {
        self.with_graph_type(crate::components::GraphType::Undirected)
    }

    /// Creates a graph with directed edges.
    #[inline(always)]
    pub fn with_directed_edges(self) -> Self {
        self.with_graph_type(crate::components::GraphType::Directed)
    }
}
