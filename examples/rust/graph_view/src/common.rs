use std::collections::HashSet;

use re_log_types::{EntityPath, EntityPathHash};
use re_viewer::external::re_types::datatypes::{GraphLocation, GraphNodeId};

use crate::{
    edge_undirected_visualizer_system::EdgeInstance, node_visualizer_system::NodeInstance,
};

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct NodeLocation {
    pub entity_hash: EntityPathHash,
    pub node_id: GraphNodeId,
}

impl From<GraphLocation> for NodeLocation {
    fn from(location: GraphLocation) -> Self {
        Self {
            entity_hash: EntityPath::from(location.entity_path).hash(),
            node_id: location.node_id,
        }
    }
}

impl std::fmt::Debug for NodeLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{:?}", self.node_id, self.entity_hash)
    }
}
