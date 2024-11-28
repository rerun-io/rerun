use re_log_types::{EntityPath, EntityPathHash};
use re_types::components;

use super::GraphNodeHash;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NodeIndex {
    pub entity_hash: EntityPathHash,
    pub node_hash: GraphNodeHash,
}

impl nohash_hasher::IsEnabled for NodeIndex {}

// We implement `Hash` manually, because `nohash_hasher` requires a single call to `state.write_*`.
// More info: https://crates.io/crates/nohash-hasher
impl std::hash::Hash for NodeIndex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // TODO(grtlr): Consider using `write_usize` here, to further decrease the risk of collision.
        let combined = self.entity_hash.hash64() << 32 | self.node_hash.hash64();
        state.write_u64(combined);
    }
}

impl NodeIndex {
    pub fn from_entity_node(entity_path: &EntityPath, node: &components::GraphNode) -> Self {
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
