use super::GraphNode;

impl GraphNode {
    /// Returns the string slice of the graph node.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<GraphNode> for String {
    #[inline]
    fn from(value: GraphNode) -> Self {
        value.as_str().to_owned()
    }
}
