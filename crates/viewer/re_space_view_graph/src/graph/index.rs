use re_log_types::{EntityPath, EntityPathHash};
use re_types::datatypes;

use super::NodeIdHash;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeIndex {
    pub entity_hash: EntityPathHash,
    pub node_id: NodeIdHash,
}

impl NodeIndex {
    pub fn new(entity_path: &EntityPath, node_id: &datatypes::GraphNode) -> Self {
        Self {
            entity_hash: entity_path.hash(),
            node_id: NodeIdHash::from(node_id),
        }
    }
}

impl std::fmt::Debug for NodeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeIndex({:?}@{:?})", self.node_id, self.entity_hash)
    }
}
