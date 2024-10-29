use re_log_types::{EntityPath, Instance};
use re_types::{components, ArrowString};

use crate::graph::NodeIndex;

impl<'a> EdgeInstance<'a> {
    pub fn nodes(&'a self) -> impl Iterator<Item = &components::GraphNode> {
        [&self.source, &self.target].into_iter()
    }
}

impl<'a> From<&NodeInstance<'a>> for NodeIndex {
    fn from(node: &NodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_hash: node.node_id.into(),
        }
    }
}

impl<'a> From<NodeInstance<'a>> for NodeIndex {
    fn from(node: NodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_hash: node.node_id.into(),
        }
    }
}

pub struct NodeInstance<'a> {
    pub node_id: &'a components::GraphNode,
    pub entity_path: &'a EntityPath,
    pub instance: Instance,
    pub label: Option<&'a ArrowString>,
    pub show_labels: bool,
    pub color: Option<egui::Color32>,
    pub position: Option<[f32; 2]>,
}

pub struct EdgeInstance<'a> {
    pub source: components::GraphNode,
    pub target: components::GraphNode,
    pub entity_path: &'a re_log_types::EntityPath,
    pub instance: Instance,
    pub edge_type: components::GraphType,
}

impl<'a> EdgeInstance<'a> {
    pub fn source_index(&self) -> NodeIndex {
        NodeIndex::from_entity_node(self.entity_path, &self.source)
    }

    pub fn target_index(&self) -> NodeIndex {
        NodeIndex::from_entity_node(self.entity_path, &self.target)
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
