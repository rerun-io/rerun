use re_log_types::{EntityPath, Instance};
use re_types::{components, datatypes, ArrowString};

use crate::graph::NodeIndex;

impl<'a> EdgeInstance<'a> {
    pub fn nodes(&'a self) -> impl Iterator<Item = &datatypes::GraphNode> {
        [self.source, self.target].into_iter()
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
    pub node_id: &'a datatypes::GraphNode,
    pub entity_path: &'a EntityPath,
    pub instance: Instance,
    pub label: Option<&'a ArrowString>,
    pub show_labels: bool,
    pub color: Option<egui::Color32>,
    pub position: Option<[f32; 2]>,
}

pub struct EdgeInstance<'a> {
    pub source: &'a datatypes::GraphNode,
    pub target: &'a datatypes::GraphNode,
    pub entity_path: &'a re_log_types::EntityPath,
    pub instance: Instance,
    pub edge_type: components::GraphType,
}

impl<'a> EdgeInstance<'a> {
    pub fn source_ix(&self) -> NodeIndex {
        NodeIndex::from_entity_node(self.entity_path, self.source)
    }

    pub fn target_ix(&self) -> NodeIndex {
        NodeIndex::from_entity_node(self.entity_path, self.target)
    }
}

pub struct UnknownNodeInstance<'a> {
    pub node_id: &'a datatypes::GraphNode,
    pub entity_path: &'a EntityPath,
}

impl<'a> From<&UnknownNodeInstance<'a>> for NodeIndex {
    fn from(node: &UnknownNodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_hash: node.node_id.into(),
        }
    }
}

impl<'a> From<UnknownNodeInstance<'a>> for NodeIndex {
    fn from(node: UnknownNodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_hash: node.node_id.into(),
        }
    }
}
