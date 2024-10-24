use crate::datatypes::{GraphEdge, GraphNode};

impl<T: Into<GraphNode>> From<(T, T)> for GraphEdge {
    fn from(value: (T, T)) -> Self {
        Self {
            source: value.0.into(),
            target: value.1.into(),
        }
    }
}
