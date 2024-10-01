use re_log_types::EntityPathHash;
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

    /// Returns the hash of the source entity, if it exists.
    pub fn source_entity_hash(&self) -> Option<EntityPathHash> {
        self.source_entity
            .as_ref()
            .map(|e| re_log_types::EntityPath::from(e.clone()).hash())
    }

    /// Returns the hash of the target entity, if it exists.
    pub fn target_entity_hash(&self) -> Option<EntityPathHash> {
        self.target_entity
            .as_ref()
            .map(|e| re_log_types::EntityPath::from(e.clone()).hash())
    }

    /// Specifies the entity in which the edge originates.
    #[inline]
    pub fn with_source_in(mut self, path: impl Into<EntityPath>) -> Self {
        self.0.source_entity = Some(path.into());
        self
    }

    /// Specifies the entity in which the edge terminates.
    #[inline]
    pub fn with_target_in(mut self, path: impl Into<EntityPath>) -> Self {
        self.0.target_entity = Some(path.into());
        self
    }
}
