use re_log_types::{EntityPath, EntityPathHash};
use re_sdk_types::components;

use super::GraphNodeHash;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId {
    pub entity_hash: EntityPathHash,
    pub node_hash: GraphNodeHash,
}

impl nohash_hasher::IsEnabled for NodeId {}

// We implement `Hash` manually, because `nohash_hasher` requires a single call to `state.write_*`.
// More info: https://crates.io/crates/nohash-hasher
impl std::hash::Hash for NodeId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let combined = self.entity_hash.hash64() ^ self.node_hash.hash64();
        state.write_u64(combined);
    }
}

impl NodeId {
    pub fn from_entity_node(entity_path: &EntityPath, node: &components::GraphNode) -> Self {
        Self {
            entity_hash: entity_path.hash(),
            node_hash: GraphNodeHash::from(node),
        }
    }
}

impl std::fmt::Debug for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeIndex({:?}@{:?})", self.node_hash, self.entity_hash)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EdgeId {
    // TODO(grtlr): Consider something more storage efficient here
    pub source: NodeId,
    pub target: NodeId,
}

impl EdgeId {
    pub fn self_edge(node: NodeId) -> Self {
        Self {
            source: node,
            target: node,
        }
    }

    pub fn is_self_edge(&self) -> bool {
        self.source == self.target
    }
}
