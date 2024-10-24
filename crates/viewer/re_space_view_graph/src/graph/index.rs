use re_log_types::{EntityPath, EntityPathHash};
use re_types::datatypes;

use super::GraphNodeHash;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NodeIndex {
    pub entity_hash: EntityPathHash,
    pub node_hash: GraphNodeHash,
}

impl NodeIndex {
    pub fn from_entity_node(entity_path: &EntityPath, node: &datatypes::GraphNode) -> Self {
        Self {
            entity_hash: entity_path.hash(),
            node_hash: GraphNodeHash::from(node),
        }
    }
}

impl std::fmt::Debug for NodeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeIndex({:?}@{:?})", self.node_hash, self.entity_hash)
    }
}
