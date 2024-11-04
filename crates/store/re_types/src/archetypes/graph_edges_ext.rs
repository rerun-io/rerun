use super::GraphEdges;

impl GraphEdges {
    /// Creates a graph with undirected edges.
    #[inline(always)]
    pub fn with_undirected_edges(mut self) -> Self {
        self.graph_type = Some(crate::components::GraphType::Undirected);
        self
    }

    /// Creates a graph with directed edges.
    #[inline(always)]
    pub fn with_directed_edges(mut self) -> Self {
        self.graph_type = Some(crate::components::GraphType::Directed);
        self
    }
}
