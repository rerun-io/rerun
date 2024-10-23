use crate::datatypes::{EntityPath, GraphEdge, GraphLocation, GraphNodeId};

impl<T: Into<GraphLocation>> From<(T, T)> for GraphEdge {
    fn from(value: (T, T)) -> Self {
        Self {
            source: value.0.into(),
            target: value.1.into(),
        }
    }
}

impl<E: Into<EntityPath>, N: Into<GraphNodeId>> From<(E, N, N)> for GraphEdge {
    fn from(value: (E, N, N)) -> Self {
        let entity_path = value.0.into();
        Self {
            source: GraphLocation {
                entity_path: entity_path.clone(),
                node_id: value.1.into(),
            },
            target: GraphLocation {
                entity_path,
                node_id: value.2.into(),
            },
        }
    }
}
