use re_log_types::{EntityPath};
use re_types::{components::{self, GraphNode}, ArrowString};

use crate::graph::NodeIndex;

pub struct NodeInstance {
    pub node: components::GraphNode,
    pub index: NodeIndex,
    pub label: Option<ArrowString>,
    pub color: Option<egui::Color32>,
    pub position: Option<egui::Pos2>,
}

pub struct EdgeInstance {
    pub source: GraphNode,
    pub target: GraphNode,
    pub source_index: NodeIndex,
    pub target_index: NodeIndex,
}

impl EdgeInstance {
    pub fn nodes(&self) -> impl Iterator<Item = &components::GraphNode> {
        [&self.source, &self.target].into_iter()
    }
}

/// This instance is used to represent nodes that were found in an edge but that were not specified explicitly in the [`GraphNodes`](crate::GraphNodes) archetype.
pub struct UnknownNodeInstance {
    pub node_id: components::GraphNode,

    /// The entity path of the edge that contained this node.
    pub entity_path: EntityPath,
}

impl From<&UnknownNodeInstance> for NodeIndex {
    fn from(node: &UnknownNodeInstance) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_hash: (&node.node_id).into(),
        }
    }
}

impl From<UnknownNodeInstance> for NodeIndex {
    fn from(node: UnknownNodeInstance) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_hash: (&node.node_id).into(),
        }
    }
}
