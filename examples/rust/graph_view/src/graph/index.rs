use re_log_types::{EntityPath, EntityPathHash};
use re_viewer::external::re_types::datatypes;

use super::NodeIdHash;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NodeIndex {
    pub entity_hash: EntityPathHash,
    pub node_id: NodeIdHash,
}

impl From<datatypes::GraphLocation> for NodeIndex {
    fn from(location: datatypes::GraphLocation) -> Self {
        Self {
            entity_hash: EntityPath::from(location.entity_path).hash(),
            node_id: NodeIdHash::from(&location.node_id),
        }
    }
}

impl From<&datatypes::GraphLocation> for NodeIndex {
    fn from(location: &datatypes::GraphLocation) -> Self {
        Self {
            entity_hash: EntityPath::from(location.entity_path.clone()).hash(),
            node_id: NodeIdHash::from(&location.node_id),
        }
    }
}

impl std::fmt::Debug for NodeIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeIndex({:?}@{:?})", self.node_id, self.entity_hash)
    }
}
