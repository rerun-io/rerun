use crate::datatypes::{EntityPath, GraphNodeId};

impl<E: Into<EntityPath>, N: Into<GraphNodeId>> From<(E, N)> for super::GraphLocation {
    fn from(value: (E, N)) -> Self {
        Self {
            entity_path: value.0.into(),
            node_id: value.1.into(),
        }
    }
}

impl std::fmt::Display for super::GraphLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.node_id, self.entity_path)
    }
}
