use re_log_types::{EntityPath, EntityPathHash, Instance};
use re_types::{datatypes, ArrowString};

use crate::graph::NodeIndex;

impl<'a> EdgeInstance<'a> {
    pub fn nodes(&'a self) -> impl Iterator<Item = datatypes::GraphNode> {
        [self.source.clone(), self.target.clone()].into_iter()
    }
}

impl<'a> From<&NodeInstance<'a>> for NodeIndex {
    fn from(node: &NodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_id: node.node_id.into(),
        }
    }
}

impl<'a> From<NodeInstance<'a>> for NodeIndex {
    fn from(node: NodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_path.hash(),
            node_id: node.node_id.into(),
        }
    }
}

pub(crate) struct NodeInstance<'a> {
    pub node_id: &'a datatypes::GraphNode,
    pub entity_path: &'a EntityPath,
    pub instance: Instance,
    pub show_labels: bool,
    pub label: Option<&'a ArrowString>,
    pub color: Option<egui::Color32>,
}

pub struct EdgeInstance<'a> {
    pub source: &'a datatypes::GraphNode,
    pub target: &'a datatypes::GraphNode,
    pub entity_path: &'a re_log_types::EntityPath,
    pub instance: Instance,
    pub color: Option<egui::Color32>,
}

pub(crate) struct UnknownNodeInstance<'a> {
    pub node_id: &'a datatypes::GraphNode,
    pub entity_hash: &'a EntityPathHash,
}

impl<'a> From<&UnknownNodeInstance<'a>> for NodeIndex {
    fn from(node: &UnknownNodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_hash.hash(),
            node_id: node.node_id.into(),
        }
    }
}

impl<'a> From<UnknownNodeInstance<'a>> for NodeIndex {
    fn from(node: UnknownNodeInstance<'a>) -> Self {
        Self {
            entity_hash: node.entity_hash.hash(),
            node_id: node.node_id.into(),
        }
    }
}
