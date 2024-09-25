use re_types_core::datatypes::EntityPath;

use crate::datatypes::{GraphEdge, GraphNodeId};

impl super::GraphEdge {
    /// Creates a new edge between two nodes.
    pub fn new(source: impl Into<GraphNodeId>, target: impl Into<GraphNodeId>) -> Self {
        Self(GraphEdge {
            source: source.into(),
            target: target.into(),
            ..Default::default()
        })
    }

    /// Specifies the entity in which the edge originates.
    pub fn with_source_in(mut self, path: impl Into<EntityPath>) -> Self {
        self.0.source_entity = Some(path.into());
        self
    }

    /// Specifies the entity in which the edge terminates.
    pub fn with_target_in(mut self, path: impl Into<EntityPath>) -> Self {
        self.0.target_entity = Some(path.into());
        self
    }
}
