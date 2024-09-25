// TODO(grtlr): improve these convenience methods

use re_types_core::datatypes::EntityPath;

use crate::datatypes::{GraphEdge, GraphNodeId};

impl super::GraphEdge {
    pub fn new(source: impl Into<GraphNodeId>, dest: impl Into<GraphNodeId>) -> Self {
        Self(vec![GraphEdge {
            source: source.into(),
            dest: dest.into(),
            source_entity: None,
            dest_entity: None,
        }])
    }

    pub fn new_global(
        (source_entity, source): (impl Into<EntityPath>, impl Into<GraphNodeId>),
        (dest_entity, dest): (impl Into<EntityPath>, impl Into<GraphNodeId>),
    ) -> Self {
        Self(vec![GraphEdge {
            source_entity: Some(source_entity.into()),
            source: source.into(),
            dest_entity: Some(dest_entity.into()),
            dest: dest.into(),
        }])
    }
}
